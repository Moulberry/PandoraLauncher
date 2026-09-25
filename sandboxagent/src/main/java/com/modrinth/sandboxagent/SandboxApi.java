package com.modrinth.sandboxagent;

import java.io.IOException;
import java.net.HttpURLConnection;
import java.net.URI;
import java.net.URL;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;

public class SandboxApi {

    private final String sandboxApiAuthorization;
    private final String baseUrl;

    public SandboxApi(String secret, int port) {
        this.sandboxApiAuthorization = "Bearer " + secret;
        this.baseUrl = "http://127.0.0.1:" + port;
    }

    public void openUri(URI uri) {
        HttpURLConnection conn = null;
        try {
            String encodedUri = URLEncoder.encode(uri.toString(), StandardCharsets.UTF_8.toString());
            conn = (HttpURLConnection) new URL(this.baseUrl + "/sandboxapi/openUri?uri="+encodedUri).openConnection();
            conn.setRequestProperty("Authorization", this.sandboxApiAuthorization);
            conn.setRequestMethod("POST");
            int responseCode = conn.getResponseCode();
            if (responseCode != HttpURLConnection.HTTP_OK) {
                System.err.println("Unable to use sandboxapi openUri, got error code " + responseCode);
            }
        } catch (IOException e) {
            e.printStackTrace();
        } finally {
            if (conn != null) {
                conn.disconnect();
            }
        }
    }

}
