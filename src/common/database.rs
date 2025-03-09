use firestore::FirestoreDb;
use serde::Serialize;
use tokio_stream::StreamExt;

#[derive(Debug, Clone)]
pub struct Database {
    pub client: FirestoreDb,
}

impl Database {
    pub async fn new() -> Self {
        let project_id = std::env::var("PROJECT_ID").expect("PROJECT_ID must be set");
        let client = match FirestoreDb::new(&project_id).await {
            Ok(client) => client,
            Err(e) => panic!("Failed to create Firestore client: {}", e),
        };
        Database { client }
    }

    // 型汎用的なCRUDの操作を実装する
    pub async fn create<T>(&self, collection: &str, key: &str, data: T) -> Result<(), String>
    where
        T: serde::Serialize
            + std::fmt::Debug
            + Clone
            + Send
            + Sync
            + Serialize
            + for<'de> serde::Deserialize<'de>,
    {
        match self
            .client
            .fluent()
            .insert()
            .into(collection)
            .document_id(key)
            .object(&data)
            .execute::<T>()
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to create document: {}", e)),
        }
    }

    pub async fn read<T>(&self, collection: &str, id: &str) -> Result<Option<T>, String>
    where
        T: serde::de::DeserializeOwned + Send + Sync,
    {
        match self
            .client
            .fluent()
            .select()
            .by_id_in(collection)
            .obj()
            .one(id)
            .await
        {
            Ok(data) => Ok(data),
            Err(e) => Err(format!("Failed to read document: {}", e)),
        }
    }

    pub async fn read_all<T>(
        &self,
        collection: &str,
        limit: Option<usize>,
    ) -> Result<Vec<T>, String>
    where
        T: serde::de::DeserializeOwned + Send + Sync,
    {
        match self
            .client
            .fluent()
            .list()
            .from(collection)
            .obj::<T>()
            .stream_all()
            .await
        {
            Ok(mut data) => {
                let mut result = Vec::new();
                while let Some(item) = data.next().await {
                    if let Some(l) = limit {
                        if result.len() >= l {
                            break;
                        }
                    }
                    result.push(item);
                }
                Ok(result)
            }
            Err(e) => Err(format!("Failed to read documents: {}", e)),
        }
    }

    pub async fn update<T>(&self, collection: &str, id: &str, data: T) -> Result<(), String>
    where
        T: serde::Serialize + Send + Sync + for<'de> serde::Deserialize<'de> + Serialize,
    {
        match self
            .client
            .fluent()
            .update()
            .in_col(collection)
            .document_id(id)
            .object(&data)
            .execute::<T>()
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to update document: {}", e)),
        }
    }

    pub async fn delete(&self, collection: &str, id: &str) -> Result<(), String> {
        match self
            .client
            .fluent()
            .delete()
            .from(collection)
            .document_id(id)
            .execute()
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to delete document: {}", e)),
        }
    }
}
