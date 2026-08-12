package com.lingxi.app.navigation

object Routes {
    const val HOME = "home"
    const val SETTINGS = "settings"
    const val CONDITION_CREATE = "condition/create"
    const val CONDITION_EDIT = "condition/edit/{id}"

    fun conditionEdit(id: Long) = "condition/edit/$id"
}
