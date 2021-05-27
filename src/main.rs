///
/// Recipe Book Backend - A small recipe server
/// Copyright (C) 2021 Austin Ray <austin@austinray.io>
///
/// This program is free software: you can redistribute it and/or modify
/// it under the terms of the GNU Affero General Public License as published
/// by the Free Software Foundation, either version 3 of the License, or
/// (at your option) any later version.
///
/// This program is distributed in the hope that it will be useful,
/// but WITHOUT ANY WARRANTY; without even the implied warranty of
/// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
/// GNU Affero General Public License for more details.
///
/// You should have received a copy of the GNU Affero General Public License
/// along with this program.  If not, see <https://www.gnu.org/licenses/>.
///
use actix_web::{App, HttpServer};
use log::{error, info};
use r2d2_sqlite::{self, SqliteConnectionManager};
use recipe_book_backend::SqliteConn;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    info!("Starting up...");

    let manager = SqliteConnectionManager::file("recipes.db")
        .with_init(|c| c.execute_batch("PRAGMA foreign_keys=1"));
    let pool = match r2d2::Pool::new(manager) {
        Ok(pool) => pool,
        Err(e) => {
            error!("Unable to create connection pool: {}", e);
            panic!("{}", e);
        }
    };

    let conn: SqliteConn = match pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("Unable to get SQLite connection: {}", e);
            panic!("{}", e);
        }
    };

    recipe_book_backend::create_expected_tables(&conn);

    HttpServer::new(move || {
        App::new()
            .data(pool.clone())
            .service(recipe_book_backend::hello)
            .service(recipe_book_backend::add)
            .service(recipe_book_backend::recipes)
            .service(recipe_book_backend::edit)
            .service(recipe_book_backend::delete)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
