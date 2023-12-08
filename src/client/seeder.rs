
pub struct Seeder {

}

impl Seeder {
    pub fn new() -> Seeder {
        Seeder {}
    }

    pub async fn run(self) {
        println!("Hello from seeder");
    }
}
