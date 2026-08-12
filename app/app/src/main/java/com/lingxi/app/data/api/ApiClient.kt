package com.lingxi.app.data.api

import com.jakewharton.retrofit2.converter.kotlinx.serialization.asConverterFactory
import com.lingxi.app.data.model.ServerConfig
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import retrofit2.Retrofit
import java.util.concurrent.TimeUnit

/**
 * Retrofit / OkHttp 工厂。
 *
 * TODO: 当前未接入；Mock 阶段不要从 ViewModel 调用。
 *  接入时：`ApiClient.create(serverConfig)` → [LingxiApi]
 */
object ApiClient {
    private val json = Json {
        ignoreUnknownKeys = true
        isLenient = true
    }

    private val okHttp = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(20, TimeUnit.SECONDS)
        .build()

    fun create(config: ServerConfig): LingxiApi {
        val retrofit = Retrofit.Builder()
            .baseUrl(config.httpBaseUrl())
            .client(okHttp)
            .addConverterFactory(json.asConverterFactory("application/json".toMediaType()))
            .build()
        return retrofit.create(LingxiApi::class.java)
    }
}
