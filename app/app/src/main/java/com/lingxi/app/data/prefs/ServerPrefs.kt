package com.lingxi.app.data.prefs

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.lingxi.app.BuildConfig
import com.lingxi.app.data.model.ServerConfig
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map

private val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "server_prefs")

class ServerPrefs(private val context: Context) {
    private object Keys {
        val HOST = stringPreferencesKey("host")
        val HTTP_PORT = intPreferencesKey("http_port")
        val WS_PORT = intPreferencesKey("ws_port")
        val USE_TLS = booleanPreferencesKey("use_tls")
        val WS_PATH = stringPreferencesKey("ws_path")
    }

    val configFlow: Flow<ServerConfig> = context.dataStore.data.map { prefs ->
        ServerConfig(
            host = prefs[Keys.HOST] ?: BuildConfig.DEFAULT_SERVER_HOST,
            httpPort = prefs[Keys.HTTP_PORT] ?: BuildConfig.DEFAULT_HTTP_PORT,
            wsPort = prefs[Keys.WS_PORT] ?: BuildConfig.DEFAULT_WS_PORT,
            useTls = prefs[Keys.USE_TLS] ?: BuildConfig.DEFAULT_USE_TLS,
            wsPath = prefs[Keys.WS_PATH] ?: BuildConfig.DEFAULT_WS_PATH,
        )
    }

    suspend fun save(config: ServerConfig) {
        context.dataStore.edit { prefs ->
            prefs[Keys.HOST] = config.host.trim()
            prefs[Keys.HTTP_PORT] = config.httpPort
            prefs[Keys.WS_PORT] = config.wsPort
            prefs[Keys.USE_TLS] = config.useTls
            prefs[Keys.WS_PATH] = config.wsPath.ifBlank { "/ws" }
        }
    }
}
