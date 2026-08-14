use pueue_lib::{Client, message::*};

use super::handle_response;
use crate::{client::style::OutputStyle, internal_prelude::*};

/// Tell the daemon to clear finished tasks for a specific group or the whole daemon.
///
/// The `successful_only` determines whether finished tasks should be removed or not.
pub async fn clean(
    client: &mut Client,
    style: &OutputStyle,
    group: Option<String>,
    successful_only: bool,
    older_than: Option<u64>,
    tail: Option<u64>,
) -> Result<()> {
    client
        .send_request(CleanRequest {
            successful_only,
            group,
            older_than,
            tail,
        })
        .await?;

    let response = client.receive_response().await?;

    handle_response(style, response)
}
