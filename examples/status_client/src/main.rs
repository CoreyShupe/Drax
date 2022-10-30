use drax::SizedVec;

#[derive(drax_derive::DraxTransport)]
pub struct V {
    sized_v: SizedVec<u8>,
    unsized_v: Vec<u8>,
}

fn main() {}


