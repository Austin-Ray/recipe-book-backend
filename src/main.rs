use actix_web::{get, post, put, web, App, Error, HttpResponse, HttpServer, Responder};
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

fn find_recipe_idx_by_id(store: &Vec<Recipe>, id: u32) -> Option<usize> {
    for (idx, recipe) in store.iter().enumerate() {
        if recipe.id == Some(id) {
            return Some(idx);
        }
    }

    None
}

fn replace_with_elem(store: &mut Vec<Recipe>, idx: usize, replacement: Recipe) {
    store[idx] = replacement;
}

#[put("/recipes/edit")]
async fn edit(recipe_json: web::Json<Recipe>, data: web::Data<AppState>) -> Result<HttpResponse, Error> {
    let mut recipes_store = match data.recipes.lock() {
      Ok(store) => store,
      Err(_) => return Ok(HttpResponse::InternalServerError().body(""))
    };

    let recipe: Recipe = recipe_json.into_inner();

    if let None = &recipe.id {
        return Ok(HttpResponse::BadRequest().body("Missing recipe ID"));
    }

    let recipe_idx: Option<usize> = find_recipe_idx_by_id(&recipes_store, recipe.id.unwrap());

    match recipe_idx {
        Some(idx) => {
            replace_with_elem(&mut recipes_store, idx, recipe);
            let json_resp = serde_json::to_string(&recipes_store[idx]);
            match json_resp {
                Ok(body) => Ok(HttpResponse::Ok().body(body)),
                Err(_) => Ok(HttpResponse::InternalServerError().body("JSON serialization error!")),
            }
        }
        None => Ok(HttpResponse::BadRequest().body("No matching recipe")),
    }
}

#[get("/recipes/all")]
async fn recipes(data: web::Data<AppState>) -> Result<HttpResponse, Error> {
    let recipes = data.recipes.lock().unwrap();

    match serde_json::to_string(&*recipes) {
        Ok(json) => Ok(HttpResponse::Ok().body(json)),
        Err(_) => Ok(HttpResponse::InternalServerError().body("Error")),
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
            .service(recipes)
            .service(edit)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
