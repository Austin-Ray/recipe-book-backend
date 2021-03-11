use actix_web::{get, post, put, web, App, Error, HttpResponse, HttpServer, Responder};
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
    let recipe = Recipe {
        id: Some(0),
        name: recipe_json.name.to_string(),
        desc: match &recipe_json.desc {
            Some(desc) => Some(desc.to_string()),
            _ => None,
        },
    };

    let conn = db.get().unwrap();
    conn.execute("INSERT INTO recipes (name, desc) VALUES (?1, ?2)", params![recipe.name, recipe.desc]).unwrap();

    Ok(HttpResponse::Ok().json(recipe))
}

fn update_recipe(conn: &SqliteConn, updated_recipe: &Recipe) -> rusqlite::Result<()> {
  let mut stmt = conn.prepare("UPDATE recipes SET name = (?1), desc = (?2) WHERE id = (?3)").unwrap();

  stmt.execute(params![updated_recipe.name, updated_recipe.desc, updated_recipe.id])?;

  Ok(())
}

#[put("/recipes/edit")]
async fn edit(recipe_json: web::Json<Recipe>, db: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let conn = db.get().unwrap();

    let recipe: Recipe = recipe_json.into_inner();

    if let None = &recipe.id {
        return Ok(HttpResponse::BadRequest().body("Missing recipe ID"));
    }

    let res = update_recipe(&conn, &recipe);
    match res {
      Ok(_) => Ok(HttpResponse::Ok().json(recipe)),
      Err(_) => Ok(HttpResponse::InternalServerError().body("ERROR"))
    }
}

#[get("/recipes/all")]
async fn recipes(db: web::Data<Pool>) -> Result<HttpResponse, Error> {
    let conn = db.get().unwrap();
    let mut stmt = conn.prepare("SELECT * FROM recipes").unwrap();

    let recipes: Vec<Recipe> = stmt.query_map(params![], |row| {
      Ok(Recipe {
        id: row.get(0).unwrap(),
        name: row.get(1).unwrap(),
        desc: row.get(2).unwrap()
      })
    })
    .unwrap()
    .map(|x| x.unwrap())
    .collect();

    Ok(HttpResponse::Ok().json(recipes))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "actix_web=info");

    let manager = SqliteConnectionManager::file("recipes.db");
    let pool  = r2d2::Pool::new(manager).unwrap();

    let conn: SqliteConn = pool.get().unwrap();
    conn.execute("CREATE TABLE IF NOT EXISTS recipes (id INTEGER PRIMARY KEY ASC, name TEXT, desc TEXT)", params![]).unwrap();

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
