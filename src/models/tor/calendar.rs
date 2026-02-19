use chrono::{Datelike, NaiveDate, Weekday};
use rusqlite::Connection;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct CalendarEvent {
    pub tor_id: i64,
    pub tor_label: String,
    pub tor_name: String,
    pub date: String,           // YYYY-MM-DD
    pub start_time: String,     // HH:MM
    pub duration_minutes: i64,
    pub location: String,
    pub cadence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meeting_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meeting_status: Option<String>, // "projected", "confirmed", etc.
}

/// A ToR with its cadence properties, used internally for meeting computation.
struct TorCadence {
    id: i64,
    name: String,
    label: String,
    meeting_cadence: String,
    cadence_day: String,
    cadence_time: String,
    cadence_duration_minutes: i64,
    default_location: String,
}

fn parse_weekday(s: &str) -> Option<Weekday> {
    match s.to_lowercase().as_str() {
        "monday" => Some(Weekday::Mon),
        "tuesday" => Some(Weekday::Tue),
        "wednesday" => Some(Weekday::Wed),
        "thursday" => Some(Weekday::Thu),
        "friday" => Some(Weekday::Fri),
        "saturday" => Some(Weekday::Sat),
        "sunday" => Some(Weekday::Sun),
        _ => None,
    }
}

/// Biweekly reference epoch: Monday 2026-01-05 (first Monday of 2026).
/// A biweekly meeting on cadence_day occurs in weeks where the ISO week number
/// has the same parity as the epoch week.
fn is_biweekly_week(date: NaiveDate) -> bool {
    let epoch = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
    let days_diff = (date - epoch).num_days();
    let weeks_diff = days_diff / 7;
    weeks_diff % 2 == 0
}

/// Compute all meeting instances for active ToRs in the given date range.
pub fn compute_meetings(
    conn: &Connection,
    start: NaiveDate,
    end: NaiveDate,
) -> rusqlite::Result<Vec<CalendarEvent>> {
    let tors = fetch_tor_cadences(conn)?;
    let mut events = Vec::new();

    for tor in &tors {
        if tor.meeting_cadence == "ad-hoc" || tor.meeting_cadence.is_empty() {
            continue;
        }

        let time = if tor.cadence_time.is_empty() {
            "09:00".to_string()
        } else {
            tor.cadence_time.clone()
        };

        let target_day = parse_weekday(&tor.cadence_day);

        let mut d = start;
        while d <= end {
            let dominated = match tor.meeting_cadence.as_str() {
                "daily" => true,
                "working_days" => matches!(
                    d.weekday(),
                    Weekday::Mon | Weekday::Tue | Weekday::Wed | Weekday::Thu | Weekday::Fri
                ),
                "weekly" => target_day.map_or(false, |wd| d.weekday() == wd),
                "biweekly" => {
                    target_day.map_or(false, |wd| d.weekday() == wd && is_biweekly_week(d))
                }
                "monthly" => {
                    // First occurrence of cadence_day in this month
                    target_day.map_or(false, |wd| {
                        if d.weekday() != wd {
                            return false;
                        }
                        // Check this is the first such weekday of the month
                        d.day() <= 7
                    })
                }
                _ => false,
            };

            if dominated {
                events.push(CalendarEvent {
                    tor_id: tor.id,
                    tor_label: tor.label.clone(),
                    tor_name: tor.name.clone(),
                    date: d.format("%Y-%m-%d").to_string(),
                    start_time: time.clone(),
                    duration_minutes: tor.cadence_duration_minutes,
                    location: tor.default_location.clone(),
                    cadence: tor.meeting_cadence.clone(),
                    meeting_id: None,
                    meeting_status: None,
                });
            }

            d = d.succ_opt().unwrap_or(d);
        }
    }

    // Add persisted meetings and merge with cadence events
    fetch_persisted_meetings(conn, start, end, &mut events)?;

    // Sort by date then start_time
    events.sort_by(|a, b| a.date.cmp(&b.date).then(a.start_time.cmp(&b.start_time)));

    Ok(events)
}

/// Fetch persisted meetings from the database and add them to the events list.
fn fetch_persisted_meetings(
    conn: &Connection,
    start: NaiveDate,
    end: NaiveDate,
    events: &mut Vec<CalendarEvent>,
) -> rusqlite::Result<()> {
    let start_str = start.format("%Y-%m-%d").to_string();
    let end_str = end.format("%Y-%m-%d").to_string();

    // First, find the relation_type_id for 'belongs_to_tor'
    let belongs_to_tor_id: i64 = {
        let mut stmt = conn.prepare(
            "SELECT id FROM entities WHERE entity_type = 'relation_type' AND name = 'belongs_to_tor'",
        )?;
        match stmt.query_row([], |row| row.get(0)) {
            Ok(id) => id,
            Err(_) => return Ok(()), // Relation type not found, skip
        }
    };

    let mut stmt = conn.prepare(
        "SELECT m.id, m.name, ep_date.value, ep_status.value, \
                t.id, t.label, ep_location.value \
         FROM entities m \
         LEFT JOIN entity_properties ep_date ON m.id = ep_date.entity_id AND ep_date.key = 'meeting_date' \
         LEFT JOIN entity_properties ep_status ON m.id = ep_status.entity_id AND ep_status.key = 'status' \
         LEFT JOIN entity_properties ep_location ON m.id = ep_location.entity_id AND ep_location.key = 'location' \
         LEFT JOIN relations r ON m.id = r.source_id AND r.relation_type_id = ?1 \
         LEFT JOIN entities t ON r.target_id = t.id \
         WHERE m.entity_type = 'meeting' AND ep_date.value >= ?2 AND ep_date.value <= ?3 \
         ORDER BY ep_date.value",
    )?;

    let meetings = stmt.query_map(
        rusqlite::params![belongs_to_tor_id, &start_str, &end_str],
        |row| {
        let meeting_id: i64 = row.get(0)?;
        let date: String = row.get(2)?;
        let status: String = row.get(3)?;
        let tor_id: Option<i64> = row.get(4).ok();
        let tor_label: Option<String> = row.get(5).ok();
        let location: String = row.get(6).unwrap_or_default();

        Ok((meeting_id, date, status, tor_id, tor_label, location))
    })?;

    for meeting in meetings {
        if let Ok((meeting_id, date, status, tor_id, tor_label, location)) = meeting {
            if let (Some(tid), Some(tlabel)) = (tor_id, tor_label) {
                // Add or update event with meeting information
                if let Some(event) = events.iter_mut().find(|e| {
                    e.tor_id == tid && e.date == date && e.meeting_id.is_none()
                }) {
                    // Update existing cadence event with meeting data
                    event.meeting_id = Some(meeting_id);
                    event.meeting_status = Some(status);
                    if !location.is_empty() {
                        event.location = location;
                    }
                } else {
                    // Add new event for persisted meeting (no matching cadence)
                    events.push(CalendarEvent {
                        tor_id: tid,
                        tor_label: tlabel.clone(),
                        tor_name: String::new(), // Not available in this query
                        date,
                        start_time: "09:00".to_string(), // Default if not in meeting properties
                        duration_minutes: 60, // Default duration
                        location,
                        cadence: String::new(),
                        meeting_id: Some(meeting_id),
                        meeting_status: Some(status),
                    });
                }
            }
        }
    }

    Ok(())
}

fn fetch_tor_cadences(conn: &Connection) -> rusqlite::Result<Vec<TorCadence>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.name, e.label, \
                COALESCE(p_cad.value, '') AS meeting_cadence, \
                COALESCE(p_day.value, '') AS cadence_day, \
                COALESCE(p_time.value, '') AS cadence_time, \
                COALESCE(p_dur.value, '60') AS cadence_duration_minutes, \
                COALESCE(p_loc.value, '') AS default_location \
         FROM entities e \
         LEFT JOIN entity_properties p_cad ON e.id = p_cad.entity_id AND p_cad.key = 'meeting_cadence' \
         LEFT JOIN entity_properties p_day ON e.id = p_day.entity_id AND p_day.key = 'cadence_day' \
         LEFT JOIN entity_properties p_time ON e.id = p_time.entity_id AND p_time.key = 'cadence_time' \
         LEFT JOIN entity_properties p_dur ON e.id = p_dur.entity_id AND p_dur.key = 'cadence_duration_minutes' \
         LEFT JOIN entity_properties p_loc ON e.id = p_loc.entity_id AND p_loc.key = 'default_location' \
         WHERE e.entity_type = 'tor' AND e.is_active = 1 \
         ORDER BY e.label",
    )?;

    let tors = stmt
        .query_map([], |row| {
            let dur_str: String = row.get("cadence_duration_minutes")?;
            let dur = dur_str.parse::<i64>().unwrap_or(60);
            Ok(TorCadence {
                id: row.get("id")?,
                name: row.get("name")?,
                label: row.get("label")?,
                meeting_cadence: row.get("meeting_cadence")?,
                cadence_day: row.get("cadence_day")?,
                cadence_time: row.get("cadence_time")?,
                cadence_duration_minutes: dur,
                default_location: row.get("default_location")?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(tors)
}
