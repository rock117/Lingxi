package com.lingxi.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.runtime.Composable
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.lingxi.app.navigation.Routes
import com.lingxi.app.condition.ConditionFormScreen
import com.lingxi.app.home.HomeScreen
import com.lingxi.app.settings.SettingsScreen
import com.lingxi.app.theme.LingxiTheme
import com.lingxi.app.viewmodel.ConditionViewModel
import com.lingxi.app.viewmodel.NotificationViewModel
import com.lingxi.app.viewmodel.SettingsViewModel

class MainActivity : ComponentActivity() {
    private val conditionViewModel: ConditionViewModel by viewModels()
    private val notificationViewModel: NotificationViewModel by viewModels()
    private val settingsViewModel: SettingsViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            LingxiTheme {
                LingxiNav(
                    conditionViewModel = conditionViewModel,
                    notificationViewModel = notificationViewModel,
                    settingsViewModel = settingsViewModel,
                )
            }
        }
    }
}

@Composable
private fun LingxiNav(
    conditionViewModel: ConditionViewModel,
    notificationViewModel: NotificationViewModel,
    settingsViewModel: SettingsViewModel,
) {
    val navController = rememberNavController()

    NavHost(navController = navController, startDestination = Routes.HOME) {
        composable(Routes.HOME) {
            HomeScreen(
                conditionViewModel = conditionViewModel,
                notificationViewModel = notificationViewModel,
                onOpenSettings = { navController.navigate(Routes.SETTINGS) },
                onCreateCondition = { navController.navigate(Routes.CONDITION_CREATE) },
                onEditCondition = { id -> navController.navigate(Routes.conditionEdit(id)) },
            )
        }
        composable(Routes.SETTINGS) {
            SettingsScreen(
                viewModel = settingsViewModel,
                onBack = { navController.popBackStack() },
            )
        }
        composable(Routes.CONDITION_CREATE) {
            ConditionFormScreen(
                viewModel = conditionViewModel,
                editId = null,
                onDone = { navController.popBackStack() },
                onBack = { navController.popBackStack() },
            )
        }
        composable(
            route = Routes.CONDITION_EDIT,
            arguments = listOf(navArgument("id") { type = NavType.LongType }),
        ) { entry ->
            val id = entry.arguments?.getLong("id")
            ConditionFormScreen(
                viewModel = conditionViewModel,
                editId = id,
                onDone = { navController.popBackStack() },
                onBack = { navController.popBackStack() },
            )
        }
    }
}
