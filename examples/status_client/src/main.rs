fn main() {
    let tag = drax_derive::nbt! {
        "string_tag" -> "String tag",
        "ctg_other_tag" -> {
            "another_compound_tag" -> 1u8,
            "an_int_tag" -> 2i32,
        }
        "list_tag" -> vec![10, 20, 30],
        "int_arr" -> vec![10i32, 20, 30],
        "long_arr" -> vec![10i64, 20, 30],
    };

    println!("{:#?}", tag);
}
