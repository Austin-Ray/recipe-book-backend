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
use log::info;
use recipe_book_backend::db::{self, Repo};
use recipe_book_backend::AppConfig;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    info!("Starting up...");

    HttpServer::new(move || {
        let repo: Box<dyn Repo> = db::create_repo(db::Backend::Sqlite);

        App::new()
            .data(AppConfig { repo })
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
