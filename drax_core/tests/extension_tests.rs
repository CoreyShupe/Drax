use drax::transport::TransportProcessorContext;
use std::io::Cursor;

/// Tests the "known good" values of a var int against the serde.
#[test]
pub fn var_int_serde_known_good() -> drax::transport::Result<()> {
    macro_rules! pairs {
        (with $process_context:ident {
            $($expected_value:literal, [$($input_vec:tt)*];)*
        }) => {$({
            let pair = ($expected_value, vec![$($input_vec)*]);
            let mut pair_cursor = Cursor::new(pair.1.clone());
            let actual =
                drax::extension::read_var_int_sync(&mut $process_context, &mut pair_cursor)?;
            assert_eq!(pair.0, actual);
            let pos = pair_cursor.position() as usize;
            assert_eq!(pair_cursor.into_inner().len(), pos);

            let actual_size = drax::extension::size_var_int(pair.0, &mut $process_context)?;
            assert_eq!(pair.1.len(), actual_size);
            let mut pair_cursor = Cursor::new(Vec::with_capacity(actual_size));
            drax::extension::write_var_int_sync(pair.0, &mut $process_context, &mut pair_cursor)?;
            assert_eq!(pair_cursor.into_inner(), pair.1);
        })*};
    }

    let mut process_context = TransportProcessorContext::new();
    pairs!(with process_context {
        25, [25];
        55324, [156, 176, 3];
        -8877777, [175, 146, 226, 251, 15];
        2147483647, [255, 255, 255, 255, 7];
        -2147483648, [128, 128, 128, 128, 8];
    });

    Ok(())
}
