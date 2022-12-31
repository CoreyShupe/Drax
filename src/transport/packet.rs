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
pub trait PacketComponent {
    type ComponentType: Sized;

    /// Decodes the packet component from the given reader.
    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>;

    /// Encodes the packet component to the given writer.
    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>>;

    fn size(input: &Self::ComponentType) -> Size;
}

/// Declares a packet component which resolves itself.
pub trait OwnedPacketComponent {
    /// Decodes the packet component from the given reader.
    fn decode_owned<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self>> + 'a>>
    where
        Self: Sized;

    /// Encodes the packet component to the given writer.
    fn encode_owned<'a, A: AsyncWrite + Unpin + ?Sized>(
        &'a self,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>>;

    fn size_owned(&self) -> Size;
}

impl<T> PacketComponent for T
where
    T: OwnedPacketComponent,
{
    type ComponentType = T;

    fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>> {
        T::decode_owned(read)
    }

    fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
        component_ref: &'a Self::ComponentType,
        write: &'a mut A,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
        T::encode_owned(component_ref, write)
    }

    fn size(input: &Self::ComponentType) -> Size {
        T::size_owned(input)
    }
}

/// A trait defining a packet component which is limited in size.
///
/// # Parameters
///
/// * `Limit` - The type which the limit should be defined as.
pub trait LimitedPacketComponent<Limit>: PacketComponent {
    /// Decodes the packet component from the given reader.
    ///
    /// # Parameters
    ///
    /// * `read` - The reader to read from.
    /// * `limit` - The maximum size of the packet component.
    fn decode_with_limit<'a, A: AsyncRead + Unpin + ?Sized>(
        read: &'a mut A,
        limit: Option<Limit>,
    ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
    where
        Limit: 'a;
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
        ($w_ident:ident, $field_name:ident @ser : $ty:ty) => {
            $crate::expand_field!(@ser $w_ident, $field_name: $ty)
        };
        ($w_ident:ident, $field_name:ident @ser ; $ty:ty) => {
            $crate::expand_field!(@ser $w_ident, $field_name; $ty)
        };
        ($w_ident:ident, $field_name:ident @ser : $__:ty
            $(: $ty:ty)?
            $(; $dty:ty)?
        ) => {
            $crate::expand_field!(@ser $w_ident, $field_name$(: $ty)?$(; $dty)?)
        };
        ($w_ident:ident, $field_name:ident @ser ; $__:ty
            $(: $ty:ty)?
            $(; $dty:ty)?
        ) => {
            $crate::expand_field!(@ser $w_ident, $field_name$(: $ty)?$(; $dty)?)
        };
        ($c_counter:ident, $d_counter:ident, $field_name:ident @size : $ty:ty) => {
            $crate::expand_field!(@size $c_counter, $d_counter, $field_name: $ty)
        };
        ($c_counter:ident, $d_counter:ident, $field_name:ident @size ; $ty:ty) => {
            $crate::expand_field!(@size $c_counter, $d_counter, $field_name; $ty)
        };
        ($c_counter:ident, $d_counter:ident, $field_name:ident @size : $__:ty
            $(: $ty:ty)?
            $(; $dty:ty)?
        ) => {
            $crate::expand_field!(@size $c_counter, $d_counter, $field_name$(: $ty)?$(; $dty)?);
        };
        ($c_counter:ident, $d_counter:ident, $field_name:ident @size ; $__:ty
            $(: $ty:ty)?
            $(; $dty:ty)?
        ) => {
            $crate::expand_field!(@size $c_counter, $d_counter, $field_name$(: $ty)?$(; $dty)?)
        };
        ($(#[$($tt:tt)*])* $enum_name:ident {
            $key_name:ident
                $(: $key_type:ty)?
                $(; $key_delegate_type:ty)?,
            match { $($key_matcher:tt)* },
            $(
                $key_matcher_case:literal $(: $static_product_type:ty)?$(; $static_product_delegate_type:ty)? as $variant_name:ident {
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
        }) => {
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

            impl $crate::prelude::OwnedPacketComponent for $enum_name {
                fn decode_owned<'a, A: $crate::prelude::AsyncRead + Unpin + ?Sized>(
                    __read: &'a mut A,
                ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::prelude::Result<Self>> + 'a>>
                where
                    Self: Sized
                {
                    Box::pin(async move {
                        $crate::expand_field!(@de __read, $key_name
                            $(: $key_type)?
                            $(; $key_delegate_type)?
                        );

                        match $($key_matcher)* {
                            $(
                            $key_matcher_case => {
                                $($(
                                $crate::expand_field!(@de __read, $field_name
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
                    __write: &'a mut A,
                ) -> std::pin::Pin<Box<dyn std::future::Future<Output = crate::prelude::Result<()>> + 'a>>
                {
                    Box::pin(async move {
                        macro_rules! expand_key_types {
                            (
                                $$write_ref:ident,
                                $$key_ref:ident
                                $$(:$$static_product_type:ty)?
                                $$(;$$static_product_delegate_type:ty)?
                            ) => {
                                $crate::enum_packet_components! {
                                    $$write_ref, $$key_ref @ser
                                    $(:$key_type)?
                                    $(;$key_delegate_type)?
                                    $$(:$$static_product_type)?
                                    $$(;$$static_product_delegate_type)?
                                }
                            }
                        }

                        match &self {
                            $(
                                Self::$variant_name $({$(
                                    $field_name,
                                )+})? => {
                                    {
                                        let key = $key_matcher_case;
                                        let key_ref = &key;
                                        expand_key_types!(
                                            __write, key_ref
                                            $(:$static_product_type)?
                                            $(;$static_product_delegate_type)?
                                        )
                                    }
                                    $($(
                                        $crate::expand_field!(@ser __write, $field_name
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

                fn size_owned(&self) -> $crate::prelude::Size
                {
                    macro_rules! expand_key_types {
                        (
                            $$constant_counter:ident,
                            $$dynamic_counter:ident,
                            $$key_ref:ident
                            $$(:$$static_product_type:ty)?
                            $$(;$$static_product_delegate_type:ty)?
                        ) => {
                            $crate::enum_packet_components! {
                                $$constant_counter, $$dynamic_counter, $$key_ref @size
                                $(:$key_type)?
                                $(;$key_delegate_type)?
                                $$(:$$static_product_type)?
                                $$(;$$static_product_delegate_type)?
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
                                let key = $key_matcher_case;
                                let key_ref = &key;
                                expand_key_types!(
                                    constant_counter, dynamic_counter, key_ref
                                    $(:$static_product_type)?
                                    $(;$static_product_delegate_type)?
                                )
                            }
                            $($(
                            $crate::expand_field!(@size constant_counter, dynamic_counter, $field_name
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
        };
    }

    #[macro_export]
    macro_rules! expand_field {
        (@ser $w_ident:ident, $field_name:ident
                $(: $field_type:ty)?
                $(; $delegate_type:ty)?) => {
            $(<$field_type as $crate::prelude::OwnedPacketComponent>::encode_owned($field_name, $w_ident))?
            $(<$delegate_type as $crate::prelude::PacketComponent>::encode($field_name, $w_ident))?
            .await?
        };
        (@de $r_ident:ident, $field_name:ident
                $(: $field_type:ty)?
                $(; $delegate_type:ty)?) => {
            let $field_name =
                $(<$field_type as $crate::prelude::OwnedPacketComponent>::decode_owned($r_ident))?
                $(<$delegate_type as $crate::prelude::PacketComponent>::decode($r_ident))?
            .await?;
        };
        (@size $c_counter:ident, $d_counter:ident, $field_name:ident
                $(: $field_type:ty)?
                $(; $delegate_type:ty)?) => {
            $(match <$field_type as $crate::prelude::OwnedPacketComponent>::size_owned($field_name))?
            $(match <$delegate_type as $crate::prelude::PacketComponent>::size($field_name))?
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
            $struct_name:ident {
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

            impl $crate::transport::packet::OwnedPacketComponent for $struct_name {
                fn decode_owned<'a, A: $crate::prelude::AsyncRead + Unpin + ?Sized>(
                    __read: &'a mut A,
                ) -> std::pin::Pin<Box<dyn std::future::Future<Output = $crate::prelude::Result<Self>> + 'a>>
                where
                    Self: Sized,
                {
                    Box::pin(async move {
                        $($(
                        $crate::expand_field!(@de __read, $field_name
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
                    __write: & 'a mut A,
                ) -> std::pin::Pin<Box<dyn std::future::Future<Output = $crate::prelude::Result<()>> + 'a>> {
                    Box::pin(async move {
                        $($(
                        {
                            let __temp = &self.$field_name;
                            $crate::expand_field!(@ser __write, __temp
                                    $(: $field_type)?
                                    $(; $delegate_type)?
                            );
                        }
                        )+)?
                        Ok(())
                    })
                }

                fn size_owned(&self) -> Size {
                    let constant_counter = 0;
                    let dynamic_counter = 0;

                    $(
                    let mut constant_counter = constant_counter;
                    let mut dynamic_counter = dynamic_counter;
                    $({
                        let __temp = & self.$field_name;
                        $crate::expand_field!(@size constant_counter, dynamic_counter, __temp
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

    impl PacketComponent for TcpShieldHeaderDelegate {
        type ComponentType = String;

        fn decode<'a, A: AsyncRead + Unpin + ?Sized>(
            read: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<Self::ComponentType>> + 'a>>
        {
            Box::pin(async move {
                let _ = read.read_var_int().await?;
                let out = String::decode(read).await?;
                let _ = u16::decode(read).await?;
                let _ = read.read_var_int().await?;
                Ok(out)
            })
        }

        fn encode<'a, A: AsyncWrite + Unpin + ?Sized>(
            component_ref: &'a Self::ComponentType,
            write: &'a mut A,
        ) -> Pin<Box<dyn Future<Output = crate::prelude::Result<()>> + 'a>> {
            Box::pin(async move {
                write.write_var_int(0).await?;
                String::encode(component_ref, write).await?;
                u16::encode(&0, write).await?;
                write.write_var_int(0x02).await?;
                Ok(())
            })
        }

        fn size(input: &Self::ComponentType) -> Size {
            match input.size_owned() {
                Size::Dynamic(x) => Size::Dynamic(x + 4),
                Size::Constant(x) => Size::Constant(x + 4),
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::delegates::VarInt;
    use crate::transport::packet::{OwnedPacketComponent, Size};
    use std::io::Cursor;

    crate::struct_packet_components! {
        #[derive(Debug, Eq, PartialEq)]
        Example {
            v_int; VarInt,
            uu: i32,
        }
    }

    crate::enum_packet_components! {
        #[derive(Debug, Eq, PartialEq)]
        ExampleEnum {
            key; VarInt,
            match {key},
            0 as Variant1 {
                v_int; VarInt,
                reg_int: i32,
            },
            1 as Variant2 {
                reg_int: i32,
                v_int; VarInt,
            }
        }
    }

    #[tokio::test]
    async fn test_decode_packet() -> crate::prelude::Result<()> {
        let mut v = vec![25, 0, 0, 0, 10];
        let mut cursor = Cursor::new(&mut v);
        let example = Example::decode_owned(&mut cursor).await?;
        let expected = Example {
            v_int: 25i32,
            uu: 10i32,
        };
        assert_eq!(example.v_int, 25);
        assert_eq!(example.uu, 10);
        assert_eq!(example, expected);
        Ok(())
    }

    #[tokio::test]
    async fn test_encode_packet() -> crate::prelude::Result<()> {
        let mut cursor = Cursor::new(vec![0; 5]);
        let example = Example {
            v_int: 25i32,
            uu: 10i32,
        };
        example.encode_owned(&mut cursor).await?;
        assert_eq!(cursor.into_inner(), vec![25, 0, 0, 0, 10]);
        Ok(())
    }

    #[tokio::test]
    async fn test_size_packet() -> crate::prelude::Result<()> {
        let example = Example {
            v_int: 25i32,
            uu: 10i32,
        };
        assert_eq!(Example::size_owned(&example), Size::Dynamic(5));
        Ok(())
    }

    #[tokio::test]
    async fn test_decode_enum_packet() -> crate::prelude::Result<()> {
        let mut v = vec![0, 25, 0, 0, 0, 10];
        let mut cursor = Cursor::new(&mut v);
        let example = ExampleEnum::decode_owned(&mut cursor).await?;
        let expected = ExampleEnum::Variant1 {
            v_int: 25,
            reg_int: 10,
        };
        assert_eq!(example, expected);

        let mut v = vec![1, 0, 0, 0, 10, 25];
        let mut cursor = Cursor::new(&mut v);
        let example = ExampleEnum::decode_owned(&mut cursor).await?;
        let expected = ExampleEnum::Variant2 {
            reg_int: 10,
            v_int: 25,
        };
        assert_eq!(example, expected);
        Ok(())
    }

    #[tokio::test]
    async fn test_encode_enum_packet() -> crate::prelude::Result<()> {
        let mut cursor = Cursor::new(vec![0; 6]);
        let example = ExampleEnum::Variant1 {
            v_int: 25,
            reg_int: 10,
        };
        example.encode_owned(&mut cursor).await?;
        assert_eq!(cursor.into_inner(), vec![0, 25, 0, 0, 0, 10]);

        let mut cursor = Cursor::new(vec![0; 6]);
        let example = ExampleEnum::Variant2 {
            reg_int: 10,
            v_int: 25,
        };
        example.encode_owned(&mut cursor).await?;
        assert_eq!(cursor.into_inner(), vec![1, 0, 0, 0, 10, 25]);
        Ok(())
    }

    #[tokio::test]
    async fn test_size_enum_packet() -> crate::prelude::Result<()> {
        let example = ExampleEnum::Variant1 {
            v_int: 25,
            reg_int: 10,
        };
        assert_eq!(ExampleEnum::size_owned(&example), Size::Dynamic(6));

        let example = ExampleEnum::Variant2 {
            reg_int: 10,
            v_int: 25,
        };
        assert_eq!(ExampleEnum::size_owned(&example), Size::Dynamic(6));
        Ok(())
    }
}
