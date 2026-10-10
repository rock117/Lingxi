package com.lingxi.app.data.api

import com.lingxi.app.data.model.Condition
import com.lingxi.app.data.model.CreateConditionRequest
import com.lingxi.app.data.model.HealthResponse
import com.lingxi.app.data.model.MsiResult
import com.lingxi.app.data.model.MttResult
import com.lingxi.app.data.model.NotificationItem
import com.lingxi.app.data.model.UpdateConditionRequest
import retrofit2.http.Body
import retrofit2.http.DELETE
import retrofit2.http.GET
import retrofit2.http.POST
import retrofit2.http.PUT
import retrofit2.http.Path
import retrofit2.http.Query

/**
 * 后端 REST 接口定义。
 *
 * 条件/通知 UI 仍走 Mock；市场情绪/中期趋势已分别调用 [sentiment] / [midTermTrend]。
 */
interface LingxiApi {
    @GET("api/health")
    suspend fun health(): HealthResponse

    @GET("api/market/sentiment")
    suspend fun sentiment(
        @Query("days") days: Int = 5,
        @Query("use_mock") useMock: Boolean = true,
    ): MsiResult

    @GET("api/market/mid-term-trend")
    suspend fun midTermTrend(
        @Query("use_mock") useMock: Boolean = true,
    ): MttResult

    @GET("api/conditions")
    suspend fun listConditions(): List<Condition>

    @GET("api/conditions/{id}")
    suspend fun getCondition(@Path("id") id: Long): Condition

    @POST("api/conditions")
    suspend fun createCondition(@Body body: CreateConditionRequest): Condition

    @PUT("api/conditions/{id}")
    suspend fun updateCondition(
        @Path("id") id: Long,
        @Body body: UpdateConditionRequest,
    ): Condition

    @DELETE("api/conditions/{id}")
    suspend fun deleteCondition(@Path("id") id: Long)

    @GET("api/notifications")
    suspend fun listNotifications(
        @Query("unread") unread: Boolean? = null,
    ): List<NotificationItem>

    @POST("api/notifications/{id}/read")
    suspend fun markRead(@Path("id") id: Long): NotificationItem
}
