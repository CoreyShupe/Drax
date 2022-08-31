use drax::transport::TransportProcessorContext;
use std::io::Cursor;

/// Tests the "known good" values of a var int against the deserializer.
#[tokio::test]
pub async fn var_int_deserializer_known_good() -> drax::transport::Result<()> {
    macro_rules! pairs {
        (with $process_context:ident {
            $($expected_value:literal, [$($input_vec:tt)*];)*
        }) => {$({
            let pair = ($expected_value, vec![$($input_vec)*]);
            let mut pair_cursor = Cursor::new(pair.1);
            let actual =
                drax::extension::read_var_int(&mut $process_context, &mut pair_cursor).await?;
            assert_eq!(pair.0, actual);
            let pos = pair_cursor.position() as usize;
            assert_eq!(pair_cursor.into_inner().len(), pos);
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
