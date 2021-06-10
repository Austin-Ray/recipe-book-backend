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
use crate::db::Repo;
use crate::{IngredientQuantity, Quantity, Recipe};
use log::error;
use r2d2_sqlite::{self, SqliteConnectionManager};
use rusqlite::params;

pub type Pool = r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>;
pub type SqliteConn = r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>;

pub fn create_repo() -> Box<dyn Repo> {
    create_repo_with_name("recipes.db")
}

pub fn create_repo_with_name(name: &str) -> Box<dyn Repo> {
    let path = std::path::Path::new(name);

    match path.parent() {
        Some(parent) => match std::fs::create_dir_all(parent) {
            Ok(()) => {}
            Err(e) => panic!("{}", e),
        },
        None => panic!("Unable to create database directory"),
    }

    let manager =
        SqliteConnectionManager::file(name).with_init(|c| c.execute_batch("PRAGMA foreign_keys=1"));
    let pool = match r2d2::Pool::new(manager) {
        Ok(pool) => pool,
        Err(e) => {
            error!("Unable to create connection pool: {}", e);
            panic!("{}", e);
        }
    };

    let repo: Box<dyn Repo> = Box::new(SqliteRepo { conn_man: pool });

    repo.setup();

    repo
}

pub struct SqliteRepo {
    conn_man: Pool,
}

impl SqliteRepo {
    fn get_conn(&self) -> SqliteConn {
        self.conn_man.get().unwrap()
    }

    pub fn create_expected_tables(&self, conn: &SqliteConn) {
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
}

impl Repo for SqliteRepo {
    fn setup(&self) {
        let conn = self.get_conn();
        self.create_expected_tables(&conn);
    }

    fn add_recipe(&self, recipe: &Recipe) -> rusqlite::Result<()> {
        let mut conn = self.get_conn();
        // do nothing right now
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

    fn update_recipe(&self, updated_recipe: &Recipe) -> rusqlite::Result<()> {
        let mut conn = self.get_conn();
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

    fn delete_recipe(&self, recipe_id: i32) -> rusqlite::Result<()> {
        let mut conn = self.get_conn();
        let tx = conn.transaction()?;
        let mut stmt = tx.prepare("DELETE FROM recipes WHERE id = (?)")?;
        stmt.execute(params![recipe_id])?;
        stmt.finalize()?;

        tx.commit()?;

        Ok(())
    }

    fn load_recipes(&self) -> rusqlite::Result<Vec<Recipe>> {
        let conn = self.get_conn();
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
}

fn load_steps(conn: &SqliteConn, recipe_id: u32) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT text FROM steps WHERE recipe_id = ?")?;

    let steps: Vec<String> = stmt
        .query_map(params![recipe_id], |row| row.get(0))?
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

mod tests {
    use super::*;
    use rand::Rng;

    #[allow(dead_code)]
    fn setup_repo() -> (Box<dyn Repo>, String) {
        let mut rng = rand::thread_rng();

        let name = format!("target/tests/recipes-{}.db", rng.gen::<u32>());

        cleanup_repo(&name);
        (create_repo_with_name(&name), name)
    }

    fn cleanup_repo(file_loc: &str) {
        match std::fs::remove_file(file_loc) {
            Ok(()) => {}
            Err(e) => error!("Testing error: {}", e),
        };
    }

    #[test]
    fn test_add() {
        let (repo, name) = setup_repo();

        let recipe = Recipe {
            id: Some(1),
            name: "Test Recipe".to_string(),
            desc: Some("Test Description".to_string()),
            steps: vec!["Step 1".to_string()],
            ingredients: vec![IngredientQuantity {
                ingredient: "Potato".to_string(),
                quantity: Quantity {
                    value: 1.0,
                    unit: "whole".to_string(),
                },
            }],
        };

        assert_eq!(Vec::<Recipe>::new(), repo.load_recipes().unwrap());
        repo.add_recipe(&recipe).unwrap();
        assert_eq!(vec![recipe.clone()], repo.load_recipes().unwrap());

        let recipe_2 = Recipe {
            id: Some(2),
            ..recipe.clone()
        };

        repo.add_recipe(&recipe_2).unwrap();
        assert_eq!(vec![recipe, recipe_2], repo.load_recipes().unwrap());

        cleanup_repo(&name);
    }

    #[test]
    fn test_delete() {
        let (repo, name) = setup_repo();

        let recipe = Recipe {
            id: Some(1),
            name: "Test Recipe".to_string(),
            desc: Some("Test Description".to_string()),
            steps: vec!["Step 1".to_string()],
            ingredients: vec![IngredientQuantity {
                ingredient: "Potato".to_string(),
                quantity: Quantity {
                    value: 1.0,
                    unit: "whole".to_string(),
                },
            }],
        };

        assert_eq!(Vec::<Recipe>::new(), repo.load_recipes().unwrap());
        repo.add_recipe(&recipe).unwrap();
        assert_eq!(vec![recipe.clone()], repo.load_recipes().unwrap());

        repo.delete_recipe(1).unwrap();
        assert_eq!(Vec::<Recipe>::new(), repo.load_recipes().unwrap());

        cleanup_repo(&name);
    }

    #[test]
    fn test_update() {
        let (repo, name) = setup_repo();

        let recipe = Recipe {
            id: Some(1),
            name: "Test Recipe".to_string(),
            desc: Some("Test Description".to_string()),
            steps: vec!["Step 1".to_string()],
            ingredients: vec![IngredientQuantity {
                ingredient: "Potato".to_string(),
                quantity: Quantity {
                    value: 1.0,
                    unit: "whole".to_string(),
                },
            }],
        };

        assert_eq!(Vec::<Recipe>::new(), repo.load_recipes().unwrap());
        repo.add_recipe(&recipe).unwrap();
        assert_eq!(vec![recipe.clone()], repo.load_recipes().unwrap());

        let recipe_2 = Recipe {
            steps: vec![],
            ..recipe.clone()
        };

        repo.update_recipe(&recipe_2).unwrap();
        assert_eq!(vec![recipe_2], repo.load_recipes().unwrap());

        cleanup_repo(&name);
    }
}
