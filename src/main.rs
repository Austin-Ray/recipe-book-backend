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
use actix_web::{delete, get, post, put, web, App, Error, HttpResponse, HttpServer, Responder};
use log::{error, info};
use r2d2_sqlite::{self, SqliteConnectionManager};
use rusqlite::params;
use serde::{Deserialize, Serialize};

type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;
type SqliteConn = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

#[derive(Serialize, Deserialize, Debug)]
struct Recipe {
    id: Option<u32>, // Used for database.
    name: String,
    desc: Option<String>,
    steps: Vec<String>,
    ingredients: Vec<IngredientQuantity>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Quantity {
    value: f64,
    unit: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct IngredientQuantity {
    ingredient: String,
    quantity: Quantity,
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("hello, world!")
}

fn add_recipe(conn: &mut SqliteConn, recipe: &Recipe) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO recipes (name, desc) VALUES (?1, ?2)",
        params![recipe.name, recipe.desc],
    )?;

    let recipe_id = tx.last_insert_rowid();

    let mut stmt = tx.prepare("INSERT INTO steps (recipe_id, text) VALUES (?1, ?2)")?;
    for step in recipe.steps.iter() {
        stmt.execute(params![recipe_id, step])?;
    }
    stmt.finalize()?;

    let mut ing_stmt = tx.prepare("INSERT INTO ingredients (name) SELECT (?1) WHERE NOT EXISTS (SELECT 1 FROM ingredients WHERE name = (?1))")?;
    let mut quantity_stmt = tx.prepare("INSERT INTO recipe_ingredients (recipe_id, ingredient_id, quantity, unit) VALUES (?1, (SELECT id FROM ingredients WHERE name = ?2), ?3, ?4)")?;
    for ing_quant in recipe.ingredients.iter() {
        ing_stmt.execute(params![ing_quant.ingredient])?;
        let quantity = &ing_quant.quantity;
        quantity_stmt.execute(params![
            recipe_id,
            ing_quant.ingredient,
            quantity.value,
            quantity.unit
        ])?;
    }

    ing_stmt.finalize()?;
    quantity_stmt.finalize()?;

    tx.commit()
}

#[post("/recipes/add")]
async fn add(recipe_json: web::Json<Recipe>, db: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let recipe = recipe_json.into_inner();
    let mut conn = match db.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("Unable to get database connection: {}", e);
            return Ok(HttpResponse::InternalServerError().body("DB error"));
        }
    };

    let res = add_recipe(&mut conn, &recipe);

    match res {
        Ok(_) => Ok(HttpResponse::Ok().json(recipe)),
        Err(e) => {
            error!("Unable to insert into database: {}", e);
            Ok(HttpResponse::InternalServerError().json("Database error"))
        }
    }
}

fn update_recipe(conn: &mut SqliteConn, updated_recipe: &Recipe) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;

    let mut stmt = tx.prepare("UPDATE recipes SET name = (?1), desc = (?2) WHERE id = (?3)")?;
    stmt.execute(params![
        updated_recipe.name,
        updated_recipe.desc,
        updated_recipe.id
    ])?;

    stmt = tx.prepare("DELETE FROM steps WHERE recipe_id = (?)")?;
    stmt.execute(params![updated_recipe.id])?;

    stmt = tx.prepare("INSERT INTO steps (recipe_id, text) VALUES (?1, ?2)")?;
    for step in updated_recipe.steps.iter() {
        stmt.execute(params![updated_recipe.id, step])?;
    }

    stmt = tx.prepare("DELETE FROM recipe_ingredients WHERE recipe_id = (?)")?;
    stmt.execute(params![updated_recipe.id])?;

    let mut ing_stmt = tx.prepare("INSERT INTO ingredients (name) SELECT (?1) WHERE NOT EXISTS (SELECT 1 FROM ingredients WHERE name = (?1))")?;
    let mut rec_ing_stmt = tx.prepare("INSERT INTO recipe_ingredients (recipe_id, ingredient_id, quantity, unit) VALUES (?1, (SELECT id FROM ingredients WHERE name = ?2), ?3, ?4)")?;

    for ing_quant in updated_recipe.ingredients.iter() {
        let quant = &ing_quant.quantity;
        ing_stmt.execute(params![ing_quant.ingredient])?;
        rec_ing_stmt.execute(params![
            updated_recipe.id,
            ing_quant.ingredient,
            quant.value,
            quant.unit
        ])?;
    }

    stmt.finalize()?;
    ing_stmt.finalize()?;
    rec_ing_stmt.finalize()?;

    tx.commit()
}

#[put("/recipes/edit")]
async fn edit(recipe_json: web::Json<Recipe>, db: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let mut conn = match db.get() {
        Ok(conn) => conn,
        Err(_) => return Ok(HttpResponse::InternalServerError().body("Database error")),
    };

    let recipe: Recipe = recipe_json.into_inner();

    if let None = &recipe.id {
        return Ok(HttpResponse::BadRequest().body("Missing recipe ID"));
    }

    let res = update_recipe(&mut conn, &recipe);
    match res {
        Ok(_) => Ok(HttpResponse::Ok().json(recipe)),
        Err(e) => {
            error!("Unable to update recipe: {}", e);
            Ok(HttpResponse::InternalServerError().body("ERROR"))
        }
    }
}

fn load_steps(conn: &SqliteConn, recipe_id: u32) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT text FROM steps WHERE recipe_id = ?")?;

    let steps: Vec<String> = stmt
        .query_map(params![recipe_id], |row| Ok(row.get(0)?))?
        .filter_map(|x| x.ok())
        .collect();

    Ok(steps)
}

fn load_ingredients(
    conn: &SqliteConn,
    recipe_id: u32,
) -> rusqlite::Result<Vec<IngredientQuantity>> {
    let mut stmt = conn.prepare("SELECT name, quantity, unit FROM recipe_ingredients LEFT JOIN ingredients ON ingredient_id = id WHERE recipe_id = ?")?;
    let ingredients = stmt
        .query_map(params![recipe_id], |row| {
            Ok(IngredientQuantity {
                ingredient: row.get(0)?,
                quantity: Quantity {
                    value: row.get(1)?,
                    unit: row.get(2)?,
                },
            })
        })?
        .filter_map(|x| x.ok())
        .collect();

    Ok(ingredients)
}

fn load_recipes(conn: &SqliteConn) -> rusqlite::Result<Vec<Recipe>> {
    let mut stmt = conn.prepare("SELECT * FROM recipes")?;
    let db_recipes = stmt
        .query_map(params![], |row| {
            Ok(Recipe {
                id: row.get(0)?,
                name: row.get(1)?,
                desc: row.get(2)?,
                steps: load_steps(&conn, row.get(0)?)?,
                ingredients: load_ingredients(&conn, row.get(0)?)?,
            })
        })?
        .filter_map(|x| x.ok())
        .collect();

    Ok(db_recipes)
}

#[get("/recipes/all")]
async fn recipes(db: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let conn = match db.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("Unable to get database connection: {}", e);
            return Ok(HttpResponse::InternalServerError().body("Database error."));
        }
    };

    let recipes = load_recipes(&conn);
    match recipes {
        Ok(recipes) => Ok(HttpResponse::Ok().json(recipes)),
        Err(e) => {
            error!("Unable to load recipes from DB: {}", e);
            Ok(HttpResponse::Ok().body("Database error."))
        }
    }
}

fn delete_recipe(conn: &mut SqliteConn, recipe_id: i32) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    let mut stmt = tx.prepare("DELETE FROM recipes WHERE id = (?)")?;
    stmt.execute(params![recipe_id])?;
    stmt.finalize()?;

    tx.commit()?;

    Ok(())
}

#[derive(Deserialize)]
struct Info {
    recipe_id: i32,
}

#[delete("/recipes/delete")]
async fn delete(db: web::Data<Pool>, info: web::Query<Info>) -> Result<HttpResponse, Error> {
    let mut conn = match db.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("Unable to get database connection: {}", e);
            return Ok(HttpResponse::InternalServerError().body("Database error."));
        }
    };

    match delete_recipe(&mut conn, info.recipe_id) {
        Ok(_) => Ok(HttpResponse::Ok().body("")),
        Err(e) => {
            error!("Unable to delete recipe ID {}: {}", info.recipe_id, e);
            Ok(HttpResponse::InternalServerError().body("Database error."))
        }
    }
}

fn create_expected_tables(conn: &SqliteConn) {
    let create_recipes = conn.execute(
        "CREATE TABLE IF NOT EXISTS recipes (id INTEGER PRIMARY KEY ASC, name TEXT, desc TEXT)",
        params![],
    );
    let _create_steps = conn.execute(
      "CREATE TABLE IF NOT EXISTS steps (recipe_id INTEGER, text TEXT, CONSTRAINT COMP_K PRIMARY KEY (recipe_id, text), FOREIGN KEY (recipe_id) REFERENCES recipes (id) ON UPDATE CASCADE ON DELETE CASCADE)",
      params![],
  );
    conn.execute("CREATE TABLE IF NOT EXISTS ingredients (id INTEGER PRIMARY KEY ASC, name TEXT NOT NULL UNIQUE)", params![]).unwrap();
    conn.execute("CREATE TABLE IF NOT EXISTS recipe_ingredients (recipe_id INTEGER, ingredient_id INTEGER, quantity REAL, unit TEXT, CONSTRAINT COMP_K PRIMARY KEY (recipe_id, ingredient_id), FOREIGN KEY(recipe_id) REFERENCES recipes (id) ON UPDATE CASCADE ON DELETE CASCADE, FOREIGN KEY (ingredient_id) REFERENCES ingredients (id) ON UPDATE CASCADE ON DELETE CASCADE);", params![]).unwrap();
    if let Err(e) = create_recipes {
        error!("Unable to create recipes table: {}", e);
        panic!("{}", e);
    }
}

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

    create_expected_tables(&conn);

    HttpServer::new(move || {
        App::new()
            .data(pool.clone())
            .service(hello)
            .service(add)
            .service(recipes)
            .service(edit)
            .service(delete)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
