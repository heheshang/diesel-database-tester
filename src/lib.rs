pub mod schema;
use std::{error::Error, thread};

use diesel::{
    pg::Pg,
    r2d2::{self, ConnectionManager},
    Connection, PgConnection, RunQueryDsl,
};
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

use tokio::runtime::Runtime;
use uuid::Uuid;

pub struct TestDb {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub dbname: String,
}

fn run_migrations(
    connection: &mut impl MigrationHarness<Pg>,
) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    connection.revert_all_migrations(MIGRATIONS)?;
    connection.run_pending_migrations(MIGRATIONS)?;
    Ok(())
}
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations/");

pub type Pool = r2d2::Pool<ConnectionManager<PgConnection>>;
impl TestDb {
    pub fn new(
        host: impl Into<String>,
        port: u16,
        user: impl Into<String>,
        password: impl Into<String>,
        _migration_path: &str,
    ) -> Self {
        let host = host.into();
        let user = user.into();
        let password = password.into();

        let uuid = Uuid::new_v4();
        let dbname = format!("test_{}", uuid);
        let dbname_clone = dbname.clone();
        let tdb = Self {
            host,
            port,
            user,
            password,
            dbname,
        };

        let server_url = tdb.server_url();

        let url = tdb.url();
        thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            rt.block_on(async move {
                let mut conn = establish_connection(&server_url);
                diesel::sql_query(format!(r#"CREATE DATABASE "{}""#, dbname_clone).as_str())
                    .execute(&mut conn)
                    .expect("Failed to create test database");

                let mut conn = establish_connection(&url);

                run_migrations(&mut conn).unwrap();
            });
        })
        .join()
        .expect("Failed to create test database");

        tdb
    }

    pub fn server_url(&self) -> String {
        if self.password.is_empty() {
            format!("postgres://{}@{}:{}", self.user, self.host, self.port)
        } else {
            format!(
                "postgres://{}:{}@{}:{}",
                self.user, self.password, self.host, self.port
            )
        }
    }

    pub fn url(&self) -> String {
        format!("{}/{}", self.server_url(), self.dbname)
    }
    pub fn pool(&self) -> Pool {
        let manager = ConnectionManager::<PgConnection>::new(self.url());
        r2d2::Pool::builder()
            .build(manager)
            .expect("Failed to create pool.")
    }
}
pub fn establish_connection(url: &str) -> PgConnection {
    PgConnection::establish(url).unwrap_or_else(|_| panic!("Error connecting to {}", url))
}

impl Drop for TestDb {
    fn drop(&mut self) {
        let server_url = self.server_url();
        let db_name = self.dbname.clone();
        thread::spawn(move || {
            let  rt = Runtime::new().unwrap();
            rt.block_on(async move {
                let  mut conn = establish_connection(&server_url);
                // terminate existing connections
                diesel::sql_query(&format!(r#"SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE  pid <> pg_backend_pid() and datname = '{}'"#,db_name))
                    .execute(&mut conn)
                    .expect("Failed to create test database");

                diesel::sql_query(format!(r#"DROP DATABASE "{}""#, db_name).as_str())
                    .execute(&mut conn)
                    .expect("Error while dropping database");
            });
        })
        .join()
        .expect("Failed to join thread");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::todos::{self, dsl::*};
    use diesel::prelude::*;
    use diesel::{AsChangeset, Identifiable, Insertable, Queryable, RunQueryDsl};
    use serde::{Deserialize, Serialize};

    #[derive(Identifiable, Serialize, Deserialize, Queryable)]
    pub struct Todo {
        id: i32,
        title: String,
        completed: Option<bool>,
        created_at: chrono::NaiveDateTime,
        updated_at: chrono::NaiveDateTime,
    }
    #[derive(Insertable, AsChangeset)]
    #[diesel(table_name = todos)]
    pub struct NewTodos {
        title: String,
        completed: Option<bool>,
        created_at: chrono::NaiveDateTime,
        updated_at: chrono::NaiveDateTime,
    }

    #[test]
    fn test_db_should_create_and_drop() {
        let tdb = TestDb::new("localhost", 15432, "postgres", "7cOPpA7dnc", "./migrations");
        let mut conn = establish_connection(&tdb.url());

        // insert todos
        let todo = NewTodos {
            title: "test".to_string(),
            completed: Some(true),
            created_at: chrono::Local::now().naive_local(),
            updated_at: chrono::Local::now().naive_local(),
        };
        diesel::insert_into(todos)
            .values(&todo)
            .execute(&mut conn)
            .expect("Failed to insert todo");

        // get todos
        let results = todos
            .limit(1)
            .load::<Todo>(&mut conn)
            .expect("Error loading todos");
        assert_eq!(results.len(), 1);
    }
}
