use std::future::Future;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Size {
    Dynamic(usize),
    Constant(usize),
}

impl std::ops::Add for Size {
    type Output = Size;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Size::Dynamic(x), Size::Dynamic(y))
            | (Size::Dynamic(x), Size::Constant(y))
            | (Size::Constant(x), Size::Dynamic(y)) => Size::Dynamic(x + y),
            (Size::Constant(x), Size::Constant(y)) => Size::Constant(x + y),
        }
    }
}

impl std::ops::Add<usize> for Size {
    type Output = Size;

    fn add(self, rhs: usize) -> Self::Output {
        match self {
            Size::Dynamic(x) | Size::Constant(x) => Size::Dynamic(x + rhs),
        }
    }
}

/// Defines a structure that can be encoded and decoded.
pub trait PacketComponent<C> {
    type ComponentType: Sized;

    /// Decodes the packet component from the given reader.
    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        context: &'a mut C,
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>;

    /// Encodes the packet component to the given writer.
    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        context: &'a mut C,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>>;

    fn size(input: &Self::ComponentType, context: &mut C) -> crate::prelude::Result<Size>;
}

#[cfg(feature = "nbt")]
pub mod nbt;
pub mod option;
pub mod primitive;
#[cfg(feature = "serde")]
pub mod serde_json;
pub mod string;
pub mod vec;

#[cfg(feature = "macros")]
pub mod macros {
    #[macro_export]
    macro_rules! enum_packet_components {
        (@internal @match $key_ident:ident) => {
            $key_ident
        };
        (@internal @match $__:ident @alt $matcher:expr) => {
            $matcher
        };
        (@internal @case $value:literal) => {
            $value
        };
        (@internal @case $__:literal @alt $value:literal) => {
            $value
        };
        ($context:ident, $w_ident:ident, $field_name:ident @ser : $ty:ty) => {
            $crate::expand_field!(@ser $context, $w_ident, $field_name: $ty)
        };
        ($context:ident, $w_ident:ident, $field_name:ident @ser ; $ty:ty) => {
            $crate::expand_field!(@ser $context, $w_ident, $field_name; $ty)
        };
        ($context:ident, $w_ident:ident, $field_name:ident @ser : $__:ty
            $(: $ty:ty)?
            $(; $dty:ty)?
        ) => {
            $crate::expand_field!(@ser $context, $w_ident, $field_name$(: $ty)?$(; $dty)?)
        };
        ($context:ident, $w_ident:ident, $field_name:ident @ser ; $__:ty
            $(: $ty:ty)?
            $(; $dty:ty)?
        ) => {
            $crate::expand_field!(@ser $context, $w_ident, $field_name$(: $ty)?$(; $dty)?)
        };
        ($context:ident, $c_counter:ident, $d_counter:ident, $field_name:ident @size : $ty:ty) => {
            $crate::expand_field!(@size $context, $c_counter, $d_counter, $field_name: $ty)
        };
        ($context:ident, $c_counter:ident, $d_counter:ident, $field_name:ident @size ; $ty:ty) => {
            $crate::expand_field!(@size $context, $c_counter, $d_counter, $field_name; $ty)
        };
        ($context:ident, $c_counter:ident, $d_counter:ident, $field_name:ident @size : $__:ty
            $(: $ty:ty)?
            $(; $dty:ty)?
        ) => {
            $crate::expand_field!(@size $context, $c_counter, $d_counter, $field_name$(: $ty)?$(; $dty)?)
        };
        ($context:ident, $c_counter:ident, $d_counter:ident, $field_name:ident @size ; $__:ty
            $(: $ty:ty)?
            $(; $dty:ty)?
        ) => {
            $crate::expand_field!(@size $context, $c_counter, $d_counter, $field_name$(: $ty)?$(; $dty)?)
        };
        ($($(#[$($tt:tt)*])* $enum_name:ident$(<$ctx_ty:ty>)? {
            $key_name:ident
                $(: $key_type:ty)?
                $(; $key_delegate_type:ty)?,
                $(@ser_ty $static_product_type:ty,)?
                $(@ser_delegate $static_product_delegate_type:ty,)?
                $(@match $key_matcher:expr,)?
            $(
                $($key_matcher_case:literal =>)? $variant_name:ident {
                    $(
                        $(
                        $field_name:ident
                            $(: $field_type:ty)?
                            $(; $delegate_type:ty)?
                        ,
                        )+
                    )?
                }
            ),*
        })*) => {$(
            $(#[$($tt)*])*
            pub enum $enum_name {
                $(
                    $variant_name$({
                        $(
                        $field_name:
                            $($field_type)?
                            $(<$delegate_type as $crate::transport::packet::PacketComponent>::ComponentType)?,
                        )+
                    })?,
                )*
            }

            $crate::expand_field!(@internal @impl_bind $enum_name, C $(@alt $ctx_ty)?)
            {
                macro_rules! ctx_type {
                    ($$alt_ident) => {
                        $crate::expand_field!(@internal @ty_bind $$alt_ident $(@alt $ctx_ty)?)
                    };
                }

                fn decode_owned<'a, A: $crate::prelude::AsyncRead + Unpin + ?Sized>(
                    __context: &'a mut ctx_type!(C),
                    __read: &'a mut A,
                ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::prelude::Result<Self>> + 'a>>
                where
                    Self: Sized
                {
                    Box::pin(async move {
                        $crate::expand_field!(@de __context, __read, $key_name
                            $(: $key_type)?
                            $(; $key_delegate_type)?
                        );

                        match $crate::enum_packet_components!(@internal @match $key_name $(@alt $key_matcher)?) {
                            $(
                            $crate::enum_packet_components!(@internal @case ${index(0)} $(@alt $key_matcher_case)?) => {
                                $($(
                                $crate::expand_field!(@de __context, __read, $field_name
                                    $(: $field_type)?
                                    $(; $delegate_type)?
                                );
                                )+)?
                                Ok(Self::$variant_name $({
                                    $($field_name,)*
                                })?)
                            }
                            )*
                            _ => $crate::throw_explain!(format!("Failed to decode key {} for type {}", $key_name, stringify!($enum_name))),
                        }
                    })
                }

                fn encode_owned<'a, A: $crate::prelude::AsyncWrite + Unpin + ?Sized>(
                    &'a self,
                    __context: &'a mut ctx_type!(C),
                    __write: &'a mut A,
                ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::prelude::Result<()>> + 'a>>
                {
                    Box::pin(async move {
                        macro_rules! expand_key_types {
                            (
                                $$write_ref:ident,
                                $$key_ref:ident,
                                $$ctx_ref:ident
                            ) => {
                                $crate::enum_packet_components! {
                                    $$ctx_ref, $$write_ref, $$key_ref @ser
                                    $(:$key_type)?
                                    $(;$key_delegate_type)?
                                    $(:$static_product_type)?
                                    $(;$static_product_delegate_type)?
                                }
                            }
                        }

                        match &self {
                            $(
                                Self::$variant_name $({$(
                                    $field_name,
                                )+})? => {
                                    {
                                        let key = $crate::enum_packet_components!(@internal @case ${index(0)} $(@alt $key_matcher_case)?);
                                        let key_ref = &key;
                                        expand_key_types!(__write, key_ref, __context);
                                    }
                                    $($(
                                        $crate::expand_field!(@ser __context, __write, $field_name
                                            $(: $field_type)?
                                            $(; $delegate_type)?
                                        );
                                    )+)?
                                    Ok(())
                                }
                            )*
                        }
                    })
                }

                fn size_owned(&self, __context: &'a mut ctx_type!(C)) -> $crate::prelude::Size
                {
                    macro_rules! expand_key_types {
                        (
                            $$constant_counter:ident,
                            $$dynamic_counter:ident,
                            $$key_ref:ident,
                            $$ctx_ref:ident
                        ) => {
                            $crate::enum_packet_components! {
                                $$ctx_ref, $$constant_counter, $$dynamic_counter, $$key_ref @size
                                $(:$key_type)?
                                $(;$key_delegate_type)?
                                $(:$static_product_type)?
                                $(;$static_product_delegate_type)?
                            }
                        }
                    }

                    let mut constant_counter = 0;
                    let mut dynamic_counter = 0;
                    match &self {
                        $(
                        Self::$variant_name $({$(
                        $field_name,
                        )+})? => {
                            {
                                let key = $crate::enum_packet_components!(@internal @case ${index(0)} $(@alt $key_matcher_case)?);
                                let key_ref = &key;
                                expand_key_types!(constant_counter, dynamic_counter, key_ref, __context);
                            }
                            $($(
                            $crate::expand_field!(@size __context, constant_counter, dynamic_counter, $field_name
                                $(: $field_type)?
                                $(; $delegate_type)?
                            );
                            )+)?
                        }
                        )*
                    }

                    if constant_counter == dynamic_counter {
                        $crate::transport::packet::Size::Constant(constant_counter)
                    } else {
                        $crate::transport::packet::Size::Dynamic(dynamic_counter)
                    }
                }
            }
        )*};
    }

    #[macro_export]
    macro_rules! expand_field {
        (@internal @impl_bind, $field_name:ident) => {
            impl<$field_name>
        };
        (@internal @impl_bind, $__:ident @alt $____:ty) => {
            impl
        };
        (@internal @ty_bind $field_name:ident) => {
            $field_name
        };
        (@internal @ty_bind $__:ident @alt $ctx_ty:ty) => {
            $ctx_ty
        };
        (@ser $context:ident, $w_ident:ident, $field_name:ident
                $(: $field_type:ty)?
                $(; $delegate_type:ty)?) => {
            $(<$field_type as $crate::prelude::OwnedPacketComponent>::encode_owned($field_name, $context, $w_ident))?
            $(<$delegate_type as $crate::prelude::PacketComponent>::encode($field_name, $context, $w_ident))?
            .await?
        };
        (@de $context:ident, $r_ident:ident, $field_name:ident
                $(: $field_type:ty)?
                $(; $delegate_type:ty)?) => {
            let $field_name =
                $(<$field_type as $crate::prelude::OwnedPacketComponent>::decode_owned($context, $r_ident))?
                $(<$delegate_type as $crate::prelude::PacketComponent>::decode($context, $r_ident))?
            .await?;
        };
        (@size $context:ident, $c_counter:ident, $d_counter:ident, $field_name:ident
                $(: $field_type:ty)?
                $(; $delegate_type:ty)?) => {
            $(match <$field_type as $crate::prelude::OwnedPacketComponent>::size_owned($field_name, $context))?
            $(match <$delegate_type as $crate::prelude::PacketComponent>::size($field_name, $context))?
            {
                $crate::transport::packet::Size::Constant(x) => {
                    $c_counter += x;
                    $d_counter += x;
                }
                $crate::transport::packet::Size::Dynamic(x) => $d_counter += x,
            }
        };
    }

    #[macro_export]
    macro_rules! struct_packet_components {
        (@internal $(#[$($tt:tt)*])* @ $struct_name:ident) => {
            $(#[$($tt)*])*
            pub struct $struct_name;
        };
        (@internal $(#[$($tt:tt)*])* @expand $(
            $field_name:ident
                $(: $field_type:ty)?
                $(; $delegate_type:ty)?,
        )+ @ $struct_name:ident) => {
            $(#[$($tt)*])*
            pub struct $struct_name {
                $(
                pub $field_name:
                    $($field_type)?
                    $(<$delegate_type as $crate::transport::packet::PacketComponent>::ComponentType)?,
                )+
            }
        };
        ($(
            $(#[$($tt:tt)*])*
            $struct_name:ident$(<$ctx_ty:ty>)? {
            $(
                $(
                $field_name:ident
                    $(: $field_type:ty)?
                    $(; $delegate_type:ty)?
                ,
                )+
            )?
        })*) => {$(
            $crate::struct_packet_components!(@internal $(#[$($tt)*])*
                $(
                    @expand $(
                        $field_name
                            $(: $field_type)?
                            $(; $delegate_type)?,
                    )+
                )?
                @ $struct_name
            );

            $crate::expand_field!(@internal @impl_bind C $(@alt $ctx_ty)?)
            $crate::prelude::OwnedPacketComponent<$crate::expand_field!(@internal @ty_bind C $(@alt $ctx_ty)?)>
            for $struct_name {
                macro_rules! ctx_type {
                    ($$alt_ident) => {
                        $crate::expand_field!(@internal @ty_bind $$alt_ident $(@alt $ctx_ty)?)
                    };
                }

                fn decode_owned<'a, A: $crate::prelude::AsyncRead + Unpin + ?Sized>(
                    __context: &'a mut ctx_type!(C),
                    __read: &'a mut A,
                ) -> std::pin::Pin<Box<dyn std::future::Future<Output = $crate::prelude::Result<Self>> + 'a>>
                where
                    Self: Sized,
                {
                    Box::pin(async move {
                        $($(
                        $crate::expand_field!(@de __context, __read, $field_name
                                $(: $field_type)?
                                $(; $delegate_type)?
                        );
                        )+)?
                        Ok(Self $({
                            $(
                            $field_name,
                            )+
                        })?)
                    })
                }

                fn encode_owned <'a, A: $crate::prelude::AsyncWrite + Unpin + ?Sized> (
                    &'a self,
                    __context: &'a mut ctx_type!(C),
                    __write: & 'a mut A,
                ) -> std::pin::Pin<Box<dyn std::future::Future<Output = $crate::prelude::Result<()>> + 'a>> {
                    Box::pin(async move {
                        $($(
                        {
                            let __temp = &self.$field_name;
                            $crate::expand_field!(@ser __context, __write, __temp
                                    $(: $field_type)?
                                    $(; $delegate_type)?
                            );
                        }
                        )+)?
                        Ok(())
                    })
                }

                fn size_owned(&self, __context: &'a mut ctx_type!(C)) -> Size {
                    let constant_counter = 0;
                    let dynamic_counter = 0;

                    $(
                    let mut constant_counter = constant_counter;
                    let mut dynamic_counter = dynamic_counter;
                    $({
                        let __temp = & self.$field_name;
                        $crate::expand_field!(@size __context, constant_counter, dynamic_counter, __temp
                                $(: $field_type)?
                                $(; $delegate_type)?
                        );
                    })+
                    )?

                    if constant_counter == dynamic_counter {
                        $crate::transport::packet::Size::Constant(constant_counter)
                    } else {
                        $crate::transport::packet::Size::Dynamic(dynamic_counter)
                    }
                }
            }
        )*};
    }
}

#[cfg(feature = "tcp-shield")]
mod tcp_shield {
    use crate::prelude::{DraxReadExt, DraxWriteExt, OwnedPacketComponent, PacketComponent};
    use crate::transport::packet::Size;
    use std::future::Future;
    use std::pin::Pin;
    use tokio::io::{AsyncRead, AsyncWrite};

    pub struct TcpShieldHeaderDelegate;

    impl<C> PacketComponent<C> for TcpShieldHeaderDelegate {
        type ComponentType = String;

        fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
            context: &'a mut C,
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
        {
            Box::pin(async move {
                let _ = read.read_var_int().await?;
                let out = String::decode(context, read).await?;
                let _ = u16::decode(context, read).await?;
                let _ = read.read_var_int().await?;
                Ok(out)
            })
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self::ComponentType,
            context: &'a mut C,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
            Box::pin(async move {
                write.write_var_int(0).await?;
                String::encode(component_ref, context, write).await?;
                u16::encode(&0, context, write).await?;
                write.write_var_int(0x02).await?;
                Ok(())
            })
        }

        fn size(input: &Self::ComponentType, context: &mut C) -> Size {
            match input.size_owned(context) {
                Size::Dynamic(x) => Size::Dynamic(x + 4),
                Size::Constant(x) => Size::Constant(x + 4),
            }
        }
    }
}

// #[cfg(test)]
// mod test {
//     use crate::delegates::VarInt;
//     use crate::prelude::PacketComponent;
//     use crate::transport::packet::{OwnedPacketComponent, Size};
//     use std::future::Future;
//     use std::io::Cursor;
//     use std::pin::Pin;
//     use tokio::io::{AsyncRead, AsyncWrite};
//
//     pub struct DelegateStr;
//
//     impl<C> PacketComponent for DelegateStr {
//         type ComponentType = &'static str;
//         type ContextType = C;
//
//         fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
//             __context: &'a mut C,
//             _read: &'a mut A,
//         ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
//         {
//             unimplemented!(
//                 "This is a delegate for key types - this cannot be used to decode values."
//             )
//         }
//
//         fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
//             component_ref: &'a Self::ComponentType,
//             context: &'a mut C,
//             write: &'a mut A,
//         ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
//             Box::pin(async move {
//                 let c_ref = component_ref.to_string();
//                 String::encode_owned(&c_ref, context, write).await?;
//                 Ok(())
//             })
//         }
//
//         fn size(input: &Self::ComponentType, context: &mut C) -> Size {
//             input.to_string().size_owned(context)
//         }
//     }
//
//     crate::struct_packet_components! {
//         #[derive(Debug, Eq, PartialEq)]
//         Example {
//             v_int; VarInt,
//             uu: i32,
//         }
//     }
//
//     crate::enum_packet_components! {
//         #[derive(Debug, Eq, PartialEq)]
//         ExampleStringEnum {
//             key: String,
//             @ser_delegate DelegateStr,
//             @match key.as_str(),
//             "example" => Variant1 {
//                 v_int; VarInt,
//                 reg_int: i32,
//             },
//             "another_variant" => Variant2 {
//                 reg_int: i32,
//                 v_int; VarInt,
//             }
//         }
//
//         #[derive(Debug, Eq, PartialEq)]
//         ExampleEnum {
//             key; VarInt,
//             Variant1 {
//                 v_int; VarInt,
//                 reg_int: i32,
//             },
//             Variant2 {
//                 reg_int: i32,
//                 v_int; VarInt,
//             }
//         }
//     }
//
//     #[tokio::test]
//     async fn test_decode_packet() -> crate::prelude::Result<()> {
//         let mut v = vec![25, 0, 0, 0, 10];
//         let mut cursor = Cursor::new(&mut v);
//         let example = Example::decode_owned(&mut cursor).await?;
//         let expected = Example {
//             v_int: 25i32,
//             uu: 10i32,
//         };
//         assert_eq!(example.v_int, 25);
//         assert_eq!(example.uu, 10);
//         assert_eq!(example, expected);
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_encode_packet() -> crate::prelude::Result<()> {
//         let mut cursor = Cursor::new(vec![0; 5]);
//         let example = Example {
//             v_int: 25i32,
//             uu: 10i32,
//         };
//         example.encode_owned(&mut cursor).await?;
//         assert_eq!(cursor.into_inner(), vec![25, 0, 0, 0, 10]);
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_size_packet() -> crate::prelude::Result<()> {
//         let example = Example {
//             v_int: 25i32,
//             uu: 10i32,
//         };
//         assert_eq!(Example::size_owned(&example), Size::Dynamic(5));
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_decode_enum_packet() -> crate::prelude::Result<()> {
//         let mut v = vec![0, 25, 0, 0, 0, 10];
//         let mut cursor = Cursor::new(&mut v);
//         let example = ExampleEnum::decode_owned(&mut cursor).await?;
//         let expected = ExampleEnum::Variant1 {
//             v_int: 25,
//             reg_int: 10,
//         };
//         assert_eq!(example, expected);
//
//         let mut v = vec![1, 0, 0, 0, 10, 25];
//         let mut cursor = Cursor::new(&mut v);
//         let example = ExampleEnum::decode_owned(&mut cursor).await?;
//         let expected = ExampleEnum::Variant2 {
//             reg_int: 10,
//             v_int: 25,
//         };
//         assert_eq!(example, expected);
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_encode_enum_packet() -> crate::prelude::Result<()> {
//         let mut cursor = Cursor::new(vec![0; 6]);
//         let example = ExampleEnum::Variant1 {
//             v_int: 25,
//             reg_int: 10,
//         };
//         example.encode_owned(&mut cursor).await?;
//         assert_eq!(cursor.into_inner(), vec![0, 25, 0, 0, 0, 10]);
//
//         let mut cursor = Cursor::new(vec![0; 6]);
//         let example = ExampleEnum::Variant2 {
//             reg_int: 10,
//             v_int: 25,
//         };
//         example.encode_owned(&mut cursor).await?;
//         assert_eq!(cursor.into_inner(), vec![1, 0, 0, 0, 10, 25]);
//         Ok(())
//     }
//
//     #[tokio::test]
//     async fn test_size_enum_packet() -> crate::prelude::Result<()> {
//         let example = ExampleEnum::Variant1 {
//             v_int: 25,
//             reg_int: 10,
//         };
//         assert_eq!(ExampleEnum::size_owned(&example), Size::Dynamic(6));
//
//         let example = ExampleEnum::Variant2 {
//             reg_int: 10,
//             v_int: 25,
//         };
//         assert_eq!(ExampleEnum::size_owned(&example), Size::Dynamic(6));
//         Ok(())
//     }
// }
