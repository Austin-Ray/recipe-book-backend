use actix_web::{get, post, put, web, App, Error, HttpResponse, HttpServer, Responder};
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

    let new_id = tx.last_insert_rowid();

    let mut stmt = tx.prepare("INSERT INTO steps (recipe_id, text) VALUES (?1, ?2)")?;
    for step in recipe.steps.iter() {
        stmt.execute(params![new_id, step])?;
    }
    stmt.finalize()?;

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
    stmt.finalize()?;

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

struct SqlRecipeStepJoin {
    id: u32,
    name: String,
    desc: String,
    text: String,
}

fn recipe_step_joins_to_recipes(joins: Vec<SqlRecipeStepJoin>) -> Vec<Recipe> {
    let mut db_recipes = vec![];
    let mut recipe_step_join_iter = joins.iter();

    // Take the first element
    let first_row = match recipe_step_join_iter.nth(0) {
        Some(elem) => elem,
        None => return vec![],
    };

    let mut cur_id = first_row.id;
    let mut cur_name = first_row.name.to_owned();
    let mut cur_desc = first_row.desc.to_owned();
    let mut cur_steps: Vec<String> = vec![];
    cur_steps.push(first_row.text.to_owned());

    for join in recipe_step_join_iter {
        if join.id != cur_id {
            let new_recipe = Recipe {
                id: Some(cur_id),
                name: String::from(cur_name),
                desc: Some(String::from(cur_desc)),
                steps: cur_steps,
            };

            cur_id = join.id;
            cur_name = join.name.to_owned();
            cur_desc = join.desc.to_owned();

            cur_steps = vec![];

            db_recipes.push(new_recipe);
        }

        cur_steps.push(join.text.to_owned());
    }

    let new_recipe = Recipe {
        id: Some(cur_id),
        name: String::from(cur_name),
        desc: Some(String::from(cur_desc)),
        steps: cur_steps,
    };
    db_recipes.push(new_recipe);

    db_recipes
}

fn load_recipes(conn: &SqliteConn) -> rusqlite::Result<Vec<Recipe>> {
    let mut stmt = conn
        .prepare("SELECT id, name, desc, text FROM recipes LEFT JOIN steps WHERE id = recipe_id")?;

    let recipe_step_join: Vec<SqlRecipeStepJoin> = stmt
        .query_map(params![], |row| {
            Ok(SqlRecipeStepJoin {
                id: row.get(0)?,
                name: row.get(1)?,
                desc: row.get(2)?,
                text: row.get(3)?,
            })
        })?
        .filter_map(|x| x.ok())
        .collect();

    let db_recipes: Vec<Recipe> = recipe_step_joins_to_recipes(recipe_step_join);

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

fn create_expected_tables(conn: &SqliteConn) {
    let create_recipes = conn.execute(
        "CREATE TABLE IF NOT EXISTS recipes (id INTEGER PRIMARY KEY ASC, name TEXT, desc TEXT)",
        params![],
    );
    let _create_steps = conn.execute(
      "CREATE TABLE IF NOT EXISTS steps (recipe_id INTEGER, text TEXT, CONSTRAINT COMP_K PRIMARY KEY (recipe_id, text), FOREIGN KEY (recipe_id) REFERENCES recipes (id))",
      params![],
  );
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
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
