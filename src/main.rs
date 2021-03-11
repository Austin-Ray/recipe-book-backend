use actix_web::{get, post, put, web, App, Error, HttpResponse, HttpServer, Responder};
use log::error;
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
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("hello, world!")
}

#[post("/recipes/add")]
async fn add(recipe_json: web::Json<Recipe>, db: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let recipe = recipe_json.into_inner();
    let conn = match db.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("Unable to get database connection: {}", e);
            return Ok(HttpResponse::InternalServerError().body("DB error"));
        }
    };

    let res = conn.execute(
        "INSERT INTO recipes (name, desc) VALUES (?1, ?2)",
        params![recipe.name, recipe.desc],
    );
    match res {
        Ok(_) => Ok(HttpResponse::Ok().json(recipe)),
        Err(e) => {
            error!("Unable to insert into database: {}", e);
            Ok(HttpResponse::InternalServerError().json("Database error"))
        }
    }
}

fn update_recipe(conn: &SqliteConn, updated_recipe: &Recipe) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare("UPDATE recipes SET name = (?1), desc = (?2) WHERE id = (?3)")?;

    stmt.execute(params![
        updated_recipe.name,
        updated_recipe.desc,
        updated_recipe.id
    ])?;

    Ok(())
}

#[put("/recipes/edit")]
async fn edit(recipe_json: web::Json<Recipe>, db: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let conn = match db.get() {
        Ok(conn) => conn,
        Err(_) => return Ok(HttpResponse::InternalServerError().body("Database error")),
    };

    let recipe: Recipe = recipe_json.into_inner();

    if let None = &recipe.id {
        return Ok(HttpResponse::BadRequest().body("Missing recipe ID"));
    }

    let res = update_recipe(&conn, &recipe);
    match res {
        Ok(_) => Ok(HttpResponse::Ok().json(recipe)),
        Err(e) => {
            error!("Unable to update recipe: {}", e);
            Ok(HttpResponse::InternalServerError().body("ERROR"))
        }
    }
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

    let mut stmt = match conn.prepare("SELECT * FROM recipes") {
        Ok(stmt) => stmt,
        Err(e) => {
            error!("Error fetching recipes from database: {}", e);
            return Ok(HttpResponse::InternalServerError().body("Database error."));
        }
    };

    let query_map_res = stmt.query_map(params![], |row| {
        Ok(Recipe {
            id: row.get(0)?,
            name: row.get(1)?,
            desc: row.get(2)?,
        })
    });

    let recipes: Vec<Recipe> = match query_map_res {
        Ok(elems) => elems.filter_map(|x| x.ok()).collect(),
        Err(e) => {
          error!("Error unwrapping recipe results: {}", e);
          return Ok(HttpResponse::Ok().body("Database error."))
        },
    };

    Ok(HttpResponse::Ok().json(recipes))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    let manager = SqliteConnectionManager::file("recipes.db");
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

    let create_res = conn.execute(
        "CREATE TABLE IF NOT EXISTS recipes (id INTEGER PRIMARY KEY ASC, name TEXT, desc TEXT)",
        params![],
    );
    if let Err(e) = create_res {
        error!("Unable to create recipes table: {}", e);
        panic!("{}", e);
    }

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
