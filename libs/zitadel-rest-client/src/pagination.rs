use std::future::Future;

use crate::error::ZitadelError;

/// Pagination request matching Zitadel Management API v2 semantics.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct PageRequest {
    pub offset: u64,
    pub limit: u32,
    pub asc: bool,
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 100,
            asc: true,
        }
    }
}

#[derive(Debug)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
}

/// Drain every page from a Zitadel list endpoint.
///
/// Stops as soon as a short page is returned OR the accumulated item count
/// reaches `total`. The loop is bounded by `total` to defeat servers that
/// report an inconsistent page size.
#[allow(dead_code)]
pub async fn list_all<T, F, Fut>(mut fetch: F) -> Result<Vec<T>, ZitadelError>
where
    F: FnMut(PageRequest) -> Fut,
    Fut: Future<Output = Result<Page<T>, ZitadelError>>,
{
    let mut out: Vec<T> = Vec::new();
    let mut req = PageRequest::default();
    loop {
        let page = fetch(req).await?;
        let got = page.items.len() as u64;
        out.extend(page.items);
        if got == 0 {
            break;
        }
        if got < req.limit as u64 {
            break;
        }
        if page.total > 0 && out.len() as u64 >= page.total {
            break;
        }
        req.offset += got;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[tokio::test]
    async fn stops_on_short_page() {
        let calls = RefCell::new(0usize);
        let got = list_all(|req| {
            *calls.borrow_mut() += 1;
            let offset = req.offset;
            async move {
                if offset == 0 {
                    Ok(Page {
                        items: (0..100).collect::<Vec<u32>>(),
                        total: 150,
                    })
                } else {
                    Ok(Page {
                        items: (100..150).collect::<Vec<u32>>(),
                        total: 150,
                    })
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(got.len(), 150);
        assert_eq!(*calls.borrow(), 2);
    }

    #[tokio::test]
    async fn stops_on_empty_page() {
        let got: Vec<u32> = list_all(|_| async move {
            Ok(Page {
                items: vec![],
                total: 0,
            })
        })
        .await
        .unwrap();
        assert!(got.is_empty());
    }

    #[tokio::test]
    async fn propagates_error() {
        let err = list_all::<u32, _, _>(|_| async move { Err(ZitadelError::from_status(500, "boom".into())) })
            .await
            .unwrap_err();
        assert!(matches!(err, ZitadelError::Internal { .. }));
    }
}
