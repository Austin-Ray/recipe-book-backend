use actix_web::{post, web, App, Error, HttpResponse, HttpServer, Responder};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

#[derive(Serialize, Deserialize, Debug)]
struct Recipe {
    id: Option<u32>, // Used for database.
    name: String,
    desc: Option<String>,
}

struct AppState {
    recipes: Mutex<Vec<Recipe>>,
}

#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("hello, world!")
}

#[post("/recipes/add")]
async fn add(recipe_json: web::Json<Recipe>, data: web::Data<AppState>) -> Result<HttpResponse, Error> {
    let recipe = Recipe {
        id: Some(0),
        name: recipe_json.name.to_string(),
        desc: match &recipe_json.desc {
            Some(desc) => Some(desc.to_string()),
            _ => None,
        },
    };

    let output_json = serde_json::to_string(&recipe);
    let recipe_store = data.recipes.lock();

    match recipe_store {
      Ok(mut store) => store.push(recipe),
      Err(_) => return Ok(HttpResponse::InternalServerError().body(""))
    }

    match output_json  {
        Ok(json) => Ok(HttpResponse::Ok().body(json)),
        Err(_) => Ok(HttpResponse::InternalServerError().body("")),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let state = web::Data::new(AppState {
      recipes: Mutex::new(vec![])
    });

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .service(hello)
            .service(add)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
