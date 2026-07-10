<?php
/*
Plugin Name: Prune Inactive Links
Description: Automatically deletes links created > 7 days ago that have not received any clicks in the last 7 days.
Version: 1.0
Author: Antigravity
*/

if ( !defined( 'YOURLS_ABSPATH' ) ) die();

// Hook into page loads to check if daily prune is needed
yourls_add_action( 'plugins_loaded', 'pil_check_prune' );

function pil_check_prune() {
    // Only run once a day
    $last_run = yourls_get_option( 'pil_last_prune_time', 0 );
    if ( ( time() - $last_run ) < 86400 ) {
        return;
    }

    pil_do_prune();
    yourls_update_option( 'pil_last_prune_time', time() );
}

// Allow manual trigger via API: ?action=prune_inactive
yourls_add_filter( 'api_action_prune_inactive', 'pil_api_prune' );
function pil_api_prune() {
    $count = pil_do_prune();
    return array(
        'statusCode' => 200,
        'simple'     => "Pruned $count inactive links successfully.",
        'message'    => "success: pruned $count links",
        'count'      => $count
    );
}

function pil_do_prune() {
    $db = yourls_get_db();
    $table_url = YOURLS_DB_TABLE_URL;
    $table_log = YOURLS_DB_TABLE_LOG;

    // Find all keywords created > 7 days ago, having no click log entry in the last 7 days
    $sql = "
        SELECT u.keyword 
        FROM `$table_url` u 
        WHERE u.timestamp < (NOW() - INTERVAL 7 DAY) 
          AND u.keyword NOT IN (
              SELECT DISTINCT l.shorturl 
              FROM `$table_log` l 
              WHERE l.click_time >= (NOW() - INTERVAL 7 DAY)
          )
    ";
    
    $results = $db->fetchObjects($sql);
    
    $keywords = [];
    if ($results) {
        foreach ($results as $row) {
            $keywords[] = $row->keyword;
        }
    }

    if (empty($keywords)) {
        return 0;
    }

    $deleted_count = 0;
    foreach ($keywords as $keyword) {
        // Use native YOURLS delete function so it handles database records and any cache clear
        if ( yourls_delete_link_by_keyword( $keyword ) ) {
            $deleted_count++;
        }
    }
    
    return $deleted_count;
}
