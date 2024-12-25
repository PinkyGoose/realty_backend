use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Создаем таблицу realtor_object
        manager
            .create_table(
                Table::create()
                    .table(RealtorObject::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RealtorObject::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(RealtorObject::Name).string().not_null())
                    .col(ColumnDef::new(RealtorObject::Phone).string().not_null())
                    .col(ColumnDef::new(RealtorObject::FullName).string().not_null())
                    .col(
                        ColumnDef::new(RealtorObject::MetroStation)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RealtorObject::MetroDistance)
                            .float()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Создаем таблицу picture
        manager
            .create_table(
                Table::create()
                    .table(Picture::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Picture::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Picture::RealtorObjectId).uuid().not_null())
                    .col(ColumnDef::new(Picture::Original).binary().not_null())
                    .col(ColumnDef::new(Picture::Thumbnail).binary().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Picture::Table, Picture::RealtorObjectId)
                            .to(RealtorObject::Table, RealtorObject::Id),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Удаляем таблицы в обратном порядке
        manager
            .drop_table(Table::drop().table(Picture::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(RealtorObject::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum RealtorObject {
    Table,
    Id,
    Name,
    Phone,
    FullName,
    MetroStation,
    MetroDistance,
}

#[derive(Iden)]
enum Picture {
    Table,
    Id,
    RealtorObjectId,
    Original,
    Thumbnail,
}
