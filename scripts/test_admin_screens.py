#!/usr/bin/env python3
"""
Admin screens GUI testing - verify functionality and capture UI
"""
from playwright.sync_api import sync_playwright
import time
import os

def test_admin_screens():
    with sync_playwright() as p:
        browser = p.chromium.launch(headless=True)
        page = browser.new_page()
        
        try:
            # Navigate to app
            print("🔍 Navigating to app...")
            page.goto('http://localhost:8080', wait_until='networkidle')
            page.wait_for_load_state('networkidle')
            
            # Check if we're on login page
            login_heading = page.locator('text=Sign In')
            if login_heading.count() > 0:
                print("✓ Login page loaded")
                page.screenshot(path='/tmp/admin_01_login.png', full_page=True)
                
                # Log in
                print("🔐 Logging in as admin...")
                page.fill('input[name="username"]', 'admin')
                page.fill('input[name="password"]', 'admin123')
                page.click('button[type="submit"]')
                page.wait_for_load_state('networkidle')
                time.sleep(1)
            
            # Dashboard
            print("📊 Testing Dashboard...")
            page.screenshot(path='/tmp/admin_02_dashboard.png', full_page=True)
            
            # Navigate to Admin > Users
            print("👥 Testing Users page...")
            page.click('text=Admin')
            page.wait_for_timeout(500)
            page.click('text=Users', exact=False)
            page.wait_for_load_state('networkidle')
            time.sleep(0.5)
            page.screenshot(path='/tmp/admin_03_users_list.png', full_page=True)
            
            # Test user creation form
            print("➕ Testing User creation form...")
            create_btn = page.locator('button:has-text("Create User"), button:has-text("New User"), a:has-text("Create"), a:has-text("New")')
            if create_btn.count() > 0:
                create_btn.first.click()
                page.wait_for_load_state('networkidle')
                time.sleep(0.5)
                page.screenshot(path='/tmp/admin_04_user_create_form.png', full_page=True)
                page.go_back()
                page.wait_for_load_state('networkidle')
            
            # Navigate to Roles
            print("🎭 Testing Roles page...")
            page.click('text=Admin')
            page.wait_for_timeout(500)
            page.click('text=Roles')
            page.wait_for_load_state('networkidle')
            time.sleep(0.5)
            page.screenshot(path='/tmp/admin_05_roles_list.png', full_page=True)
            
            # Test role creation form
            print("➕ Testing Role creation form...")
            create_btn = page.locator('button:has-text("Create Role"), button:has-text("New Role"), a:has-text("Create"), a:has-text("New")')
            if create_btn.count() > 0:
                create_btn.first.click()
                page.wait_for_load_state('networkidle')
                time.sleep(0.5)
                page.screenshot(path='/tmp/admin_06_role_create_form.png', full_page=True)
                page.go_back()
                page.wait_for_load_state('networkidle')
            
            # Navigate to Settings
            print("⚙️ Testing Settings page...")
            page.click('text=Admin')
            page.wait_for_timeout(500)
            settings_link = page.locator('text=Settings')
            if settings_link.count() > 0:
                settings_link.click()
                page.wait_for_load_state('networkidle')
                time.sleep(0.5)
                page.screenshot(path='/tmp/admin_07_settings.png', full_page=True)
            
            # Navigate to Ontology
            print("🧬 Testing Ontology page...")
            page.click('text=Admin')
            page.wait_for_timeout(500)
            ontology_link = page.locator('text=Ontology')
            if ontology_link.count() > 0:
                ontology_link.click()
                page.wait_for_load_state('networkidle')
                time.sleep(1)
                page.screenshot(path='/tmp/admin_08_ontology.png', full_page=True)
            
            # Navigate to Workflow Builder
            print("🔄 Testing Workflow Builder...")
            page.click('text=Governance')
            page.wait_for_timeout(500)
            workflow_link = page.locator('text=Workflow')
            if workflow_link.count() > 0:
                workflow_link.click()
                page.wait_for_load_state('networkidle')
                time.sleep(0.5)
                page.screenshot(path='/tmp/admin_09_workflow_builder.png', full_page=True)
            
            # Check menu builder if available
            print("🗂️ Testing Menu Builder...")
            page.click('text=Admin')
            page.wait_for_timeout(500)
            menu_link = page.locator('text=Menu')
            if menu_link.count() > 0:
                menu_link.click()
                page.wait_for_load_state('networkidle')
                time.sleep(0.5)
                page.screenshot(path='/tmp/admin_10_menu_builder.png', full_page=True)
            
            # Data Manager
            print("📦 Testing Data Manager...")
            page.click('text=Admin')
            page.wait_for_timeout(500)
            # Data Manager might be under a different section, try to find it
            page.screenshot(path='/tmp/admin_11_admin_menu.png', full_page=True)
            
            print("\n✅ GUI testing complete!")
            print("Screenshots saved to /tmp/admin_*.png")
            
        finally:
            browser.close()

if __name__ == '__main__':
    test_admin_screens()
