use rgf::App;

fn main() {
    let args = rgf::get_args();
    println!("{:?}", args);

    let mut app = App::new(args);

    if let Err(err) = app.run() {
        eprintln!("Error: {err}");
    }
}
