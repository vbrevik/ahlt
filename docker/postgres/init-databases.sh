#!/bin/bash
set -e

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" <<-EOSQL
    CREATE DATABASE ahlt_dev;
    CREATE DATABASE ahlt_staging;
    CREATE DATABASE ahlt_prod;
    CREATE DATABASE ahlt_test;
EOSQL
