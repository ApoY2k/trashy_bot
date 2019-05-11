use crate::schema::twitchuser;
use diesel::prelude::*;

#[derive(Identifiable, Queryable, Debug)]
#[primary_key(twitch_id)] 
pub struct TwitchUser {
    pub twitch_id: i64, 
    pub view_count: i64,
    pub display_name: String,
    pub profile_image_url: String,
    pub is_live: bool,
    pub game_id: i64,
    pub title: String,
    pub thumbnail_url: String,
}

#[derive(Insertable)]
#[table_name = "twitchuser"]
pub struct NewTwitchUser {
    twitch_id: i64, 
    view_count: i64,
    display_name: String,
    profile_image_url: String,
    is_live: bool,
    game_id: i64,
    title: String,
    thumbnail_url: String,

}

pub fn create_twitchuser(
    conn: &PgConnection,
    twitch_id: i64, 
    view_count: i64,
    display_name: String,
    profile_image_url: String,
    is_live: bool,
    game_id: i64,
    title: String,
    thumbnail_url: String,

) -> TwitchUser {
    use crate::schema::twitchuser;

    let new_twitchuser = NewTwitchUser {
        twitch_id,
        view_count,
        display_name,
        profile_image_url,
        is_live,
        game_id,
        title,
        thumbnail_url,
    };

    diesel::insert_into(twitchuser::table)
        .values(&new_twitchuser)
        .get_result(conn)
        .expect("Error saving TwitchUser")
}
