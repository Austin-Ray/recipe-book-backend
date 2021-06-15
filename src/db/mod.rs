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
use crate::Recipe;
use anyhow::Result;

mod sqlite;

pub trait Repo {
    fn setup(&self);
    fn add_recipe(&self, recipe: &Recipe) -> Result<()>;
    fn delete_recipe(&self, recipe_id: i32) -> Result<()>;
    fn update_recipe(&self, updated_recipe: &Recipe) -> Result<()>;
    fn load_recipes(&self) -> Result<Vec<Recipe>>;
}

pub enum Backend {
    Sqlite,
}

pub fn create_repo(db_backend: Backend) -> Box<dyn Repo> {
    match db_backend {
        Backend::Sqlite => sqlite::create_repo(),
    }
}
