package com.lingxi.app.navigation

object Routes {
    const val HOME = "home"
    const val SETTINGS = "settings"
    const val CONDITION_CREATE = "condition/create"
    const val CONDITION_EDIT = "condition/edit/{id}"
    const val SCREEN_COMPOSE = "screen/compose/{typeId}"
    const val SCREEN_EDIT = "screen/edit/{id}"
    const val SCREEN_RESULT = "screen/result/{conditionId}"
    const val STOCK_DETAIL = "stock/detail/{symbol}"
    const val STOCK_ANALYZE = "stock/analyze/{symbol}"

    fun conditionEdit(id: Long) = "condition/edit/$id"

    fun screenCompose(typeId: String? = null): String {
        val t = typeId?.takeIf { it.isNotBlank() } ?: "_"
        return "screen/compose/$t"
    }

    fun screenEdit(id: Long) = "screen/edit/$id"

    fun screenResult(conditionId: Long? = null): String {
        val id = conditionId ?: 0L
        return "screen/result/$id"
    }

    /** 路径中 `.` 换成 `~`，避免 Nav 路径解析问题。 */
    fun stockDetail(symbol: String): String =
        "stock/detail/${symbol.replace('.', '~')}"

    fun stockAnalyze(symbol: String): String =
        "stock/analyze/${symbol.replace('.', '~')}"

    fun decodeStockSymbol(encoded: String): String =
        encoded.replace('~', '.')
}
