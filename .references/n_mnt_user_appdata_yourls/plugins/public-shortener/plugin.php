<?php
/*
Plugin Name: Public Shortener Front Page
Description: Renders a premium, Turnstile-secured public URL shortener on the home page. Reuses keys from the Turnstile plugin automatically.
Version: 1.0
Author: Antigravity
*/

if ( !defined( 'YOURLS_ABSPATH' ) ) die();

// Hook into the custom public front page event
yourls_add_action( 'public_front_page', 'ps_render_front_page' );

// Auto-install index.php when the plugin is activated
yourls_add_action( 'activated_public-shortener/plugin.php', 'ps_install_index' );

function ps_install_index() {
    $index_path = YOURLS_ABSPATH . '/index.php';
    
    // Check if index.php already contains our action hook
    if ( file_exists( $index_path ) ) {
        $content = file_get_contents( $index_path );
        if ( strpos( $content, 'public_front_page' ) !== false ) {
            return; // Already set up
        }
    }
    
    // Write index.php trigger file
    $trigger_code = <<<'PHP'
<?php
// Load YOURLS bootstrap
define( 'YOURLS_ABSPATH', __DIR__ );
require_once( __DIR__ . '/includes/load-yourls.php' );

// Trigger public front page action hook
yourls_do_action( 'public_front_page' );

// Fallback if no plugin is active to handle the front page
?>
<!DOCTYPE html>
<html>
<head>
    <title>YOURLS</title>
</head>
<body style="font-family: sans-serif; text-align: center; padding: 50px;">
    <h1>Public Front Page</h1>
    <p>Please activate the <strong>Public Shortener Front Page</strong> plugin in the admin panel to enable the public shortener interface.</p>
</body>
</html>
PHP;

    @file_put_contents( $index_path, $trigger_code );
}

function ps_render_front_page() {
    $error = '';
    $shorturl = '';

    // Automatically inherit site/secret keys from the Cloudflare Turnstile plugin if defined
    $site_key = defined('CF_TS_SITE_KEY') ? CF_TS_SITE_KEY : '';
    $secret_key = defined('CF_TS_SECRET_KEY') ? CF_TS_SECRET_KEY : '';

    if ( isset( $_POST['url'] ) && !empty( $_POST['url'] ) ) {
        $url = trim($_POST['url']);
        
        // Cloudflare Turnstile validation (only if keys are set up)
        $valid = true;
        if (!empty($secret_key)) {
            $turnstile_response = isset($_POST['cf-turnstile-response']) ? $_POST['cf-turnstile-response'] : '';
            
            $ch = curl_init('https://challenges.cloudflare.com/turnstile/v0/siteverify');
            curl_setopt($ch, CURLOPT_POST, true);
            curl_setopt($ch, CURLOPT_RETURNTRANSFER, true);
            curl_setopt($ch, CURLOPT_POSTFIELDS, http_build_query([
                'response' => $turnstile_response,
                'secret'   => $secret_key,
            ]));
            $res = curl_exec($ch);
            curl_close($ch);
            
            $response = json_decode($res, true);
            if ( !$response || !isset($response['success']) || $response['success'] !== true ) {
                $error = 'Security check failed. Please verify you are human.';
                $valid = false;
            }
        }

        if ($valid) {
            $keyword = isset( $_POST['keyword'] ) ? trim( $_POST['keyword'] ) : '';
            $title   = '';
            
            // Shorten the link using YOURLS core function
            $result = yourls_add_new_link( $url, $keyword, $title );
            
            if ( isset( $result['status'] ) && $result['status'] === 'success' ) {
                $shorturl = $result['shorturl'];
            } else {
                $error = isset( $result['message'] ) ? $result['message'] : 'An error occurred while shortening the link.';
            }
        }
    }
    ?>
    <!DOCTYPE html>
    <html lang="en">
    <head>
        <meta charset="UTF-8">
        <meta name="viewport" content="width=device-width, initial-scale=1.0">
        <title>Shorten URL - <?php echo htmlspecialchars(yourls_get_option('site_name', 'YOURLS')); ?></title>
        <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@300;400;600;800&display=swap" rel="stylesheet">
        <?php if (!empty($site_key)): ?>
            <script src="https://challenges.cloudflare.com/turnstile/v0/api.js" async defer></script>
        <?php endif; ?>
        <style>
            :root {
                --bg-gradient: linear-gradient(135deg, #0f172a 0%, #1e1b4b 100%);
                --panel-bg: rgba(30, 41, 59, 0.45);
                --panel-border: rgba(255, 255, 255, 0.08);
                --text-primary: #f8fafc;
                --text-secondary: #94a3b8;
                --accent: #6366f1;
                --accent-hover: #4f46e5;
                --success: #10b981;
                --error: #ef4444;
            }

            * {
                box-sizing: border-box;
                margin: 0;
                padding: 0;
            }

            body {
                font-family: 'Outfit', sans-serif;
                background: var(--bg-gradient);
                color: var(--text-primary);
                min-height: 100vh;
                display: flex;
                align-items: center;
                justify-content: center;
                padding: 20px;
                overflow-x: hidden;
            }

            .container {
                width: 100%;
                max-width: 540px;
                background: var(--panel-bg);
                backdrop-filter: blur(16px);
                -webkit-backdrop-filter: blur(16px);
                border: 1px solid var(--panel-border);
                border-radius: 24px;
                padding: 40px 30px;
                box-shadow: 0 20px 40px rgba(0, 0, 0, 0.3);
                text-align: center;
                position: relative;
            }

            h1 {
                font-size: 2.5rem;
                font-weight: 800;
                margin-bottom: 8px;
                background: linear-gradient(to right, #a5b4fc, #6366f1);
                -webkit-background-clip: text;
                -webkit-text-fill-color: transparent;
            }

            .subtitle {
                font-size: 1rem;
                color: var(--text-secondary);
                margin-bottom: 30px;
            }

            form {
                display: flex;
                flex-direction: column;
                gap: 18px;
            }

            .input-group {
                text-align: left;
            }

            label {
                font-size: 0.85rem;
                font-weight: 600;
                color: var(--text-secondary);
                margin-bottom: 6px;
                display: block;
                text-transform: uppercase;
                letter-spacing: 0.05em;
            }

            input[type="text"], input[type="url"] {
                width: 100%;
                padding: 14px 18px;
                background: rgba(15, 23, 42, 0.6);
                border: 1px solid var(--panel-border);
                border-radius: 12px;
                color: var(--text-primary);
                font-family: inherit;
                font-size: 1rem;
                transition: all 0.2s ease;
            }

            input[type="text"]:focus, input[type="url"]:focus {
                outline: none;
                border-color: var(--accent);
                box-shadow: 0 0 0 3px rgba(99, 102, 241, 0.2);
            }

            .turnstile-container {
                display: flex;
                justify-content: center;
                margin: 5px 0;
            }

            button {
                width: 100%;
                padding: 16px;
                background: var(--accent);
                border: none;
                border-radius: 12px;
                color: white;
                font-size: 1.1rem;
                font-weight: 600;
                cursor: pointer;
                transition: all 0.2s ease;
                box-shadow: 0 4px 12px rgba(99, 102, 241, 0.3);
            }

            button:hover {
                background: var(--accent-hover);
                transform: translateY(-1px);
            }

            button:active {
                transform: translateY(0);
            }

            .error-message {
                background: rgba(239, 68, 68, 0.1);
                border: 1px solid rgba(239, 68, 68, 0.2);
                color: var(--error);
                padding: 14px;
                border-radius: 12px;
                font-size: 0.95rem;
                text-align: left;
                margin-bottom: 20px;
            }

            .success-container {
                background: rgba(16, 185, 129, 0.1);
                border: 1px solid rgba(16, 185, 129, 0.2);
                padding: 24px;
                border-radius: 16px;
                margin-bottom: 25px;
                animation: fadeIn 0.4s ease;
            }

            @keyframes fadeIn {
                from { opacity: 0; transform: translateY(10px); }
                to { opacity: 1; transform: translateY(0); }
            }

            .success-title {
                color: var(--success);
                font-weight: 600;
                font-size: 1.1rem;
                margin-bottom: 12px;
                display: flex;
                align-items: center;
                justify-content: center;
                gap: 8px;
            }

            .short-url-output {
                display: flex;
                gap: 10px;
            }

            .short-url-output input {
                flex-grow: 1;
                padding: 12px 16px;
                background: rgba(15, 23, 42, 0.8);
                border: 1px solid rgba(16, 185, 129, 0.3);
                border-radius: 10px;
                color: var(--text-primary);
                font-weight: 600;
                text-align: center;
                font-size: 1.1rem;
            }

            .copy-btn {
                background: var(--success);
                color: white;
                border: none;
                padding: 12px 20px;
                border-radius: 10px;
                font-weight: 600;
                cursor: pointer;
                box-shadow: 0 4px 10px rgba(16, 185, 129, 0.2);
                transition: all 0.2s ease;
            }

            .copy-btn:hover {
                opacity: 0.9;
                transform: translateY(-1px);
            }

            .admin-link {
                display: inline-block;
                margin-top: 25px;
                color: var(--text-secondary);
                font-size: 0.85rem;
                text-decoration: none;
                transition: color 0.2s ease;
            }

            .admin-link:hover {
                color: var(--accent);
            }
        </style>
    </head>
    <body>
        <div class="container">
            <h1>Shorten Link</h1>
            <p class="subtitle">Quickly shorten and optimize your long web URLs</p>

            <?php if ( !empty($error) ): ?>
                <div class="error-message">
                    <strong>Error:</strong> <?php echo htmlspecialchars($error); ?>
                </div>
            <?php endif; ?>

            <?php if ( !empty($shorturl) ): ?>
                <div class="success-container">
                    <div class="success-title">
                        <svg width="20" height="20" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">
                            <circle cx="10" cy="10" r="10" fill="#10B981" fill-opacity="0.2"/>
                            <path d="M14 7L8.5 12.5L6 10" stroke="#10B981" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>
                        </svg>
                        URL Shortened Successfully!
                    </div>
                    <div class="short-url-output">
                        <input type="text" id="shortUrlField" value="<?php echo htmlspecialchars($shorturl); ?>" readonly>
                        <button class="copy-btn" onclick="copyShortUrl()">Copy</button>
                    </div>
                </div>
            <?php endif; ?>

            <form method="post">
                <div class="input-group">
                    <label for="url">Enter Long URL</label>
                    <input type="url" id="url" name="url" placeholder="https://example.com/very-long-link-here..." required>
                </div>
                
                <div class="input-group">
                    <label for="keyword">Custom Keyword (Optional)</label>
                    <input type="text" id="keyword" name="keyword" placeholder="e.g. mylink">
                </div>

                <?php if (!empty($site_key)): ?>
                    <div class="turnstile-container">
                        <div class="cf-turnstile" data-sitekey="<?php echo htmlspecialchars($site_key); ?>"></div>
                    </div>
                <?php endif; ?>

                <button type="submit">Shorten URL</button>
            </form>

            <a class="admin-link" href="/admin/">Manage Links &rarr;</a>
        </div>

        <script>
            function copyShortUrl() {
                var copyText = document.getElementById("shortUrlField");
                copyText.select();
                copyText.setSelectionRange(0, 99999);
                navigator.clipboard.writeText(copyText.value);
                
                var btn = document.querySelector(".copy-btn");
                var originalText = btn.innerHTML;
                btn.innerHTML = "Copied!";
                btn.style.background = "#059669";
                setTimeout(function() {
                    btn.innerHTML = originalText;
                    btn.style.background = "";
                }, 2000);
            }
        </script>
    </body>
    </html>
    <?php
    exit(); // Stop execution after rendering the front page
}
