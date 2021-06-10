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
use actix_web::{delete, get, post, put, web, Error, HttpResponse, Responder};
use log::error;
use serde::{Deserialize, Serialize};

pub mod db;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Recipe {
    pub id: Option<u32>, // Used for database.
    pub name: String,
    pub desc: Option<String>,
    pub steps: Vec<String>,
    pub ingredients: Vec<IngredientQuantity>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Quantity {
    pub value: f64,
    pub unit: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct IngredientQuantity {
    pub ingredient: String,
    pub quantity: Quantity,
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("hello, world!")
}

pub struct AppConfig {
    pub repo: Box<dyn db::Repo>,
}

#[post("/recipes/add")]
async fn add(
    recipe_json: web::Json<Recipe>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse, Error> {
    let recipe = recipe_json.into_inner();
    let res = config.repo.add_recipe(&recipe);

    match res {
        Ok(_) => Ok(HttpResponse::Ok().json(recipe)),
        Err(e) => {
            error!("Unable to insert into database: {}", e);
            Ok(HttpResponse::InternalServerError().json("Database error"))
        }
    }
}

#[put("/recipes/edit")]
async fn edit(
    recipe_json: web::Json<Recipe>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse, Error> {
    let recipe: Recipe = recipe_json.into_inner();

    if recipe.id.is_none() {
        return Ok(HttpResponse::BadRequest().body("Missing recipe ID"));
    }

    let res = config.repo.update_recipe(&recipe);
    match res {
        Ok(_) => Ok(HttpResponse::Ok().json(recipe)),
        Err(e) => {
            error!("Unable to update recipe: {}", e);
            Ok(HttpResponse::InternalServerError().body("ERROR"))
        }
    }
}

#[get("/recipes/all")]
async fn recipes(config: web::Data<AppConfig>) -> Result<HttpResponse, Error> {
    let recipes = config.repo.load_recipes();
    match recipes {
        Ok(recipes) => Ok(HttpResponse::Ok().json(recipes)),
        Err(e) => {
            error!("Unable to load recipes from DB: {}", e);
            Ok(HttpResponse::Ok().body("Database error."))
        }
    }
}

#[derive(Deserialize)]
struct Info {
    recipe_id: i32,
}

#[delete("/recipes/delete")]
async fn delete(
    config: web::Data<AppConfig>,
    info: web::Query<Info>,
) -> Result<HttpResponse, Error> {
    match config.repo.delete_recipe(info.recipe_id) {
        Ok(_) => Ok(HttpResponse::Ok().body("")),
        Err(e) => {
            error!("Unable to delete recipe ID {}: {}", info.recipe_id, e);
            Ok(HttpResponse::InternalServerError().body("Database error."))
        }
    }
}
