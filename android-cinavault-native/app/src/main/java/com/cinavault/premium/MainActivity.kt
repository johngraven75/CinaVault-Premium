package com.cinavault.premium

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.core.view.WindowCompat
import com.cinavault.premium.state.CinaVaultViewModel
import com.cinavault.premium.ui.CinaVaultApp
import com.cinavault.premium.ui.theme.CinaVaultTheme

class MainActivity : ComponentActivity() {
    private val viewModel: CinaVaultViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        WindowCompat.setDecorFitsSystemWindows(window, false)

        setContent {
            CinaVaultTheme {
                CinaVaultApp(viewModel = viewModel)
            }
        }
    }
}
