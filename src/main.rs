#![feature(proc_macro_hygiene, decl_macro)]
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
#[macro_use]
extern crate log;
#[macro_use]
extern crate rocket_contrib;

mod task;
#[cfg(test)]
mod tests;

use diesel::SqliteConnection;
use rocket::fairing::AdHoc;
use rocket::request::{FlashMessage, Form};
use rocket::response::{Flash, Redirect};
use rocket::Rocket;
use rocket_contrib::{serve::StaticFiles, templates::Template};

//use crate::task::{Task, Todo}; // ou use task::{Task, Todo}=>  ou use self::task::{Task, Todo};
//quand use; chemin jusqu'à l item DEPUIS le crate
use self::task::{Task, Todo};

// Au compile time: lit les migrations et créé un module embedded_migrations.
// Au run time: les execute par la commande:embedded_migrations::run(&*conn)
// => lance les (nouvelles) migrations lors de l'attach : càd avant que l'application ne soit disponible
//https://docs.rs/diesel_migrations/1.4.0/diesel_migrations/macro.embed_migrations.html
embed_migrations!();
// La macro #[database] ajoute le trait FromRequest (=>request guard: paramètre d'entrée d'une handler sauf si dans la route attribute) à la Struct
// ET lui crée 2 fonctions:
// Self::fairing() -> Fairing : initialise la connexion dans le pool de connexion à cette base
// Self::get_one(&Rocket) -> Renvoie Some<Self> tant que Self::fairing() a bien été "attached" et qu'il y a au moins une connexion dans le pool
// https://api.rocket.rs/v0.4/rocket_contrib/databases/index.html
#[database("sqlite_database")]
pub struct DbConn(SqliteConnection);

#[derive(Debug, serde::Serialize)]
struct Context<'a> {
    msg: Option<(&'a str, &'a str)>,
    tasks: Vec<Task>,
}

impl<'a> Context<'a> {
    pub fn err(conn: &DbConn, msg: &'a str) -> Context<'a> {
        Context {
            msg: Some(("error", msg)),
            tasks: Task::all(conn),
        }
    }

    pub fn raw(conn: &DbConn, msg: Option<(&'a str, &'a str)>) -> Context<'a> {
        Context{msg: msg, tasks: Task::all(conn)}
    }
}

#[post("/", data = "<todo_form>")]
fn new(todo_form: Form<Todo>, conn: DbConn) -> Flash<Redirect> {
    let todo = todo_form.into_inner();
    if todo.description.is_empty() {
        Flash::error(Redirect::to("/"), "Description cannot be empty.")
    } else if Task::insert(todo, &conn) {
        Flash::success(Redirect::to("/"), "Todo successfully added.")
    } else {
        Flash::error(Redirect::to("/"), "Whoops! The server failed.")
    }
}

#[put("/<id>")]
fn toggle(id: i32, conn: DbConn) -> Result<Redirect, Template> {
    if Task::toggle_with_id(id, &conn) {
        Ok(Redirect::to("/"))
    } else {
        Err(Template::render("index", &Context::err(&conn, "Couldn't toggle task.")))
    }
}

#[delete("/<id>")]
fn delete(id: i32, conn: DbConn) -> Result<Flash<Redirect>, Template> {
    if Task::delete_with_id(id, &conn) {
        Ok(Flash::success(Redirect::to("/"), "Todo was deleted."))
    } else {
        Err(Template::render("index", &Context::err(&conn, "Couldn't delete task.")))
    }
}

#[get("/")]
fn index(msg: Option<FlashMessage<'_, '_>>, conn: DbConn) -> Template {
    Template::render(
        "index",
        match msg {
            Some(ref msg) => Context::raw(&conn, Some((msg.name(), msg.msg()))),
            None => Context::raw(&conn, None),
        },
    )
}
// execute les migrations.
fn run_db_migrations(rocket: Rocket) -> Result<Rocket, Rocket> {
    let conn = DbConn::get_one(&rocket).expect("no database connection");
    match embedded_migrations::run(&*conn) {
        Ok(()) => Ok(rocket),
        Err(e) => {
            error!("Failed to run database migrations: {:?}", e);
            Err(rocket)
        }
    }
}

fn rocket() -> Rocket {
    rocket::ignite()
        //nécéssaire à la création du request guard
        .attach(DbConn::fairing())
        // "run_db_migrations": juste le nom de la fonction pour la trouver.. https://doc.rust-lang.org/beta/core/ops/trait.FnOnce.html
        // avantage: le paramètre n'a pas besoin d'être connu ici. Il le sera à l'éxécution
        // "on_attach"=> exécuté immédiatement
        .attach(AdHoc::on_attach("Database Migrations", run_db_migrations))
        .mount("/", StaticFiles::from("static/")) // renvoie le path absolu
        .mount("/", routes![index])
        .mount("/todo", routes![new, toggle, delete])
        .attach(Template::fairing())
        .attach(AdHoc::on_launch("msg coucou", |_| {
            println!("coucou les amiches")
        }))
}

fn main() {
    rocket().launch();
}
