
pub struct Seeder {

}

impl Seeder {
    async fn new() -> Seeder {
        Seeder {}
    }

    pub async fn run(self) {
        println!("Hello from seeder");
    }
}
