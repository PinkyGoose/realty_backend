pub mod entities;

use axum::extract::Path;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    routing::post,
    Router,
};
use dotenv::dotenv;
use entities::{picture, realtor_object};
use image::{load_from_memory, ImageFormat};
use sea_orm::ColumnTrait;
use sea_orm::QueryFilter;
use sea_orm::TransactionTrait;
use sea_orm::{ActiveModelTrait, Database, DatabaseConnection, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use std::env;
use std::io::Cursor;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;
#[derive(Debug, Serialize, Deserialize)]
pub struct CreatedEntity {
    pub id: Uuid,
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("неверный UUID")]
    InvalidUuid,
    #[error("не найдено (что?)")]
    InternalServer,
}

impl From<Error> for serde_json::Value {
    fn from(val: Error) -> Self {
        #[rustfmt::skip] // некрасиво форматирует
        let (code, descr) = match &val {
            Error::InvalidUuid                                           => ("E_INVALID_UUID",        String::default()),
            Error::InternalServer=> ("E_INTERNAL",String::default()),
            };

        serde_json::json!({
            "code": code,
            "description": descr
        })
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (code, text): (_, serde_json::Value) = match self {
            Error::InvalidUuid => (StatusCode::BAD_REQUEST, self.into()),
            Error::InternalServer => (StatusCode::INTERNAL_SERVER_ERROR, self.into()),
        };

        (code, text.to_string()).into_response()
    }
}

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtorObjectDetails {
    id: Uuid,
    name: String,
    phone: String,
    full_name: String,
    metro_station: String,
    metro_distance: f32,
    images: Vec<String>, // Base64-encoded original images
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtorObjectSummary {
    id: Uuid,
    name: String,
    metro_station: String,
    metro_distance: f32,
    thumbnails: Vec<String>, // Base64-encoded thumbnails
}

pub async fn list_realtor_objects(
    State(state): State<AppState>,
) -> Result<Json<Vec<RealtorObjectSummary>>, Error> {
    let realtor_objects = realtor_object::Entity::find()
        .find_with_related(picture::Entity)
        .all(&state.db)
        .await
        .map_err(|_| Error::InternalServer)?;

    let result = realtor_objects
        .into_iter()
        .map(|(object, pictures)| RealtorObjectSummary {
            id: object.id,
            name: object.name,
            metro_station: object.metro_station,
            metro_distance: object.metro_distance,
            thumbnails: pictures
                .into_iter()
                .map(|p| base64::encode(p.thumbnail))
                .collect(),
        })
        .collect();

    Ok(Json(result))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealtorObjectPayload {
    name: String,
    phone: String,
    full_name: String,
    metro_station: String,
    metro_distance: f32,
    images: Vec<String>, // Base64-encoded images
}

pub async fn create_realtor_object(
    State(state): State<AppState>,
    Json(payload): Json<RealtorObjectPayload>,
) -> Result<Json<CreatedEntity>, Error> {
    let txn = state.db.begin().await.map_err(|_| Error::InternalServer)?;

    // Сохраняем риелторский объект
    let realtor_object = realtor_object::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(payload.name),
        phone: Set(payload.phone),
        full_name: Set(payload.full_name),
        metro_station: Set(payload.metro_station),
        metro_distance: Set(payload.metro_distance),
    }
    .insert(&txn)
    .await
    .map_err(|_| Error::InternalServer)?;

    // Сохраняем изображения
    for image_base64 in payload.images {
        let original = base64::decode(&image_base64).map_err(|_| Error::InternalServer)?;
        let thumbnail = compress_image(&original, 200, 200);

        let new_picture = picture::ActiveModel {
            id: Set(Uuid::new_v4()),
            realtor_object_id: Set(realtor_object.id),
            original: Set(original),
            thumbnail: Set(thumbnail),
        };

        new_picture
            .insert(&txn)
            .await
            .map_err(|_| Error::InternalServer)?;
    }

    txn.commit().await.map_err(|_| Error::InternalServer)?;

    Ok(Json(CreatedEntity {
        id: realtor_object.id,
    }))
}
pub async fn get_realtor_object(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<RealtorObjectDetails>, Error> {
    let object = realtor_object::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| Error::InternalServer)?
        .ok_or(Error::InternalServer)?;

    let pictures = picture::Entity::find()
        .filter(picture::Column::RealtorObjectId.eq(id))
        .all(&state.db)
        .await
        .map_err(|_| Error::InternalServer)?;

    Ok(Json(RealtorObjectDetails {
        id: object.id,
        name: object.name,
        phone: object.phone,
        full_name: object.full_name,
        metro_station: object.metro_station,
        metro_distance: object.metro_distance,
        images: pictures
            .into_iter()
            .map(|p| base64::encode(p.original))
            .collect(),
    }))
}
fn compress_image(data: &[u8], max_width: u32, max_height: u32) -> Vec<u8> {
    let img = load_from_memory(data).expect("Invalid image data");
    let thumbnail = img.thumbnail(max_width, max_height);

    let mut buffer = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut buffer, ImageFormat::Png)
        .expect("Failed to write image");

    buffer.into_inner()
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db = Database::connect(&database_url).await.unwrap();
    let cors = CorsLayer::new()
        .allow_origin(Any) // Разрешаем запросы с любого источника
        .allow_methods(Any) // Разрешенные HTTP-методы
        .allow_headers(Any); // Разрешаем любые заголовки

    let state = AppState { db };

    let app = Router::new()
        .route("/realtor_objects", post(create_realtor_object))
        .route("/realtor_objects", get(list_realtor_objects))
        .route("/realtor_objects/:id", get(get_realtor_object))
        .with_state(state)
        .layer(cors);

    println!("Server running on http://localhost:4000");
    match TcpListener::bind("0.0.0.0:4000").await {
        Ok(tcp_listener) => {
            axum::serve(tcp_listener, app.into_make_service())
                .await
                .unwrap();
        }
        Err(err) => {
            tracing::error!(
                ?err,
                "не удалось привязаться к порту. выход из приложения: {:?}",
                err
            );
        }
    };
}
