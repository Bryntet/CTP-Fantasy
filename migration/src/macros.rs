#[macro_export]
macro_rules! drop_table {
    ($table:ident, $manager:ident) => {
        $manager
            .drop_table(Table::drop().table($table::Table).to_owned())
            .await?
    };
}

#[macro_export]
macro_rules! drop_type {
    ($type:ident, $manager:ident) => {
        $manager
            .drop_type(Type::drop().name($type::Table).to_owned())
            .await?
    };
}

pub(crate) use drop_table;
pub(crate) use drop_type;
