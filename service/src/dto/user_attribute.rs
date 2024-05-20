use crate::dto::User;
use rocket_okapi::JsonSchema;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use std::default::Default;
use std::fmt::Debug;

#[derive(Debug)]
pub struct UserDataCombination<Data: serde::Serialize + JsonSchema + Debug + AttributeName> {
    pub user: User,
    pub data: Data,
}

pub trait AttributeName {
    const NAME: &'static str;
    const FIELD_NAME: &'static str;
}

impl<Data: serde::Serialize + rocket_okapi::JsonSchema + Debug + AttributeName> Serialize
    for UserDataCombination<Data>
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut user_combination = serializer.serialize_struct(Data::NAME, 2)?;
        user_combination.serialize_field("user", &self.user)?;
        user_combination.serialize_field(Data::FIELD_NAME, &self.data)?;
        user_combination.end()
    }
}

impl<Data: serde::Serialize + rocket_okapi::JsonSchema + Debug + AttributeName> JsonSchema
    for UserDataCombination<Data>
{
    fn schema_name() -> String {
        Data::NAME.to_string()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        let object_schema = gen.subschema_for::<User>();
        let data_schema = gen.subschema_for::<Data>();

        let mut schema_object = schemars::schema::SchemaObject {
            instance_type: Some(schemars::schema::InstanceType::Object.into()),
            object: Some(Box::new(schemars::schema::ObjectValidation {
                properties: schemars::Map::new(),
                required: std::collections::BTreeSet::from([
                    "user".to_string(),
                    Data::FIELD_NAME.to_lowercase(),
                ]),
                ..Default::default()
            })),
            ..Default::default()
        };

        schema_object
            .object
            .as_mut()
            .unwrap()
            .properties
            .insert("user".to_owned(), object_schema);

        schema_object
            .object
            .as_mut()
            .unwrap()
            .properties
            .insert(Data::FIELD_NAME.to_lowercase(), data_schema);

        schemars::schema::Schema::Object(schema_object)
    }
}
#[macro_export]
macro_rules! make_dto_user_attribute {
    ($name:ident,$val:ty) => {
        paste::paste! {
            #[derive(Debug, serde::Serialize, rocket_okapi::okapi::schemars::JsonSchema)]
            pub struct [<Attribute $name>]($val);
        }
        impl crate::dto::AttributeName for paste::paste! {[<Attribute $name>]} {
            const NAME: &'static str = concat!("UserWith", stringify!($name));
            const FIELD_NAME: &'static str = stringify!($name);
        }

        impl From<$val> for paste::paste! {[<Attribute $name>]} {
            fn from(value: $val) -> Self {
                Self(value)
            }
        }
    };
}
