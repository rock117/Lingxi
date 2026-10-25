package com.lingxi.app.data.prefs

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.lingxi.app.data.model.StockSearchHit
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.map
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

private val Context.analyzeHistoryStore: DataStore<Preferences> by preferencesDataStore(
    name = "analyze_history",
)

/** 分析页搜索/选股历史（本地，最近优先）。 */
class AnalyzeHistoryPrefs(private val context: Context) {
    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
    }

    private object Keys {
        val ITEMS = stringPreferencesKey("items_json")
    }

    val historyFlow: Flow<List<StockSearchHit>> = context.analyzeHistoryStore.data.map { prefs ->
        decode(prefs[Keys.ITEMS])
    }

    suspend fun remember(symbol: String, name: String, code: String = "", namePinyin: String = "") {
        val sym = symbol.trim()
        if (sym.isEmpty()) return
        val entry = StockSearchHit(
            symbol = sym,
            name = name.trim(),
            code = code.trim().ifBlank { sym.substringBefore('.') },
            namePinyin = namePinyin.trim(),
        )
        context.analyzeHistoryStore.edit { prefs ->
            val cur = decode(prefs[Keys.ITEMS]).filterNot {
                it.symbol.equals(sym, ignoreCase = true)
            }
            val next = (listOf(entry) + cur).take(MAX_ITEMS)
            prefs[Keys.ITEMS] = json.encodeToString(next)
        }
    }

    suspend fun remove(symbol: String) {
        val sym = symbol.trim()
        if (sym.isEmpty()) return
        context.analyzeHistoryStore.edit { prefs ->
            val next = decode(prefs[Keys.ITEMS]).filterNot {
                it.symbol.equals(sym, ignoreCase = true)
            }
            prefs[Keys.ITEMS] = json.encodeToString(next)
        }
    }

    suspend fun clear() {
        context.analyzeHistoryStore.edit { prefs ->
            prefs[Keys.ITEMS] = "[]"
        }
    }

    private fun decode(raw: String?): List<StockSearchHit> {
        if (raw.isNullOrBlank()) return emptyList()
        return runCatching {
            json.decodeFromString<List<StockSearchHit>>(raw)
        }.getOrDefault(emptyList()).filter { it.symbol.isNotBlank() }
    }

    companion object {
        const val MAX_ITEMS = 20
    }
}
