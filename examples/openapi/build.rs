// Recompile when a migration is added, so `sqlx::migrate!()` embeds it.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
