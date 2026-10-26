package com.easygo.http

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.TextView
import android.widget.Toast
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat

class MainActivity : AppCompatActivity() {

    private lateinit var statusText: TextView
    private lateinit var addrInput: EditText
    private lateinit var startButton: Button
    private lateinit var stopButton: Button

    // Android 13+ 运行时通知权限请求
    private val requestPermissionLauncher =
        registerForActivityResult(ActivityResultContracts.RequestPermission()) { granted ->
            if (granted) {
                doStartService()
            } else {
                Toast.makeText(this, "通知权限被拒绝，服务仍会启动但无持久通知", Toast.LENGTH_LONG).show()
                doStartService()
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        statusText  = findViewById(R.id.statusText)
        addrInput   = findViewById(R.id.addrInput)
        startButton = findViewById(R.id.startButton)
        stopButton  = findViewById(R.id.stopButton)

        startButton.setOnClickListener { onStartClicked() }
        stopButton.setOnClickListener  { onStopClicked() }

        getSharedPreferences("easygo", MODE_PRIVATE)
            .getString("addr", null)
            ?.takeIf { it.isNotBlank() }
            ?.let { addrInput.setText(it) }

        statusText.text = "⏹ 服务未启动"
        startButton.isEnabled = true
        stopButton.isEnabled = false
    }

    // ── 按钮事件 ─────────────────────────────────────────────────────────────

    private fun onStartClicked() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            // Android 13+ 需要运行时权限
            when {
                ContextCompat.checkSelfPermission(
                    this, Manifest.permission.POST_NOTIFICATIONS
                ) == PackageManager.PERMISSION_GRANTED -> doStartService()

                else -> requestPermissionLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
        } else {
            doStartService()
        }
    }

    private fun doStartService() {
        val addr = addrInput.text.toString().trim().ifEmpty {
            EasyGoService.DEFAULT_ADDR
        }

        getSharedPreferences("easygo", MODE_PRIVATE).edit().putString("addr", addr).apply()

        val intent = Intent(this, EasyGoService::class.java).apply {
            putExtra(EasyGoService.EXTRA_ADDR, addr)
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(intent)
        } else {
            startService(intent)
        }

        statusText.text = "✅ 服务运行中\n监听地址：$addr\n\n其他设备可通过此地址调用 API"
        startButton.isEnabled = false
        stopButton.isEnabled  = true
    }

    private fun onStopClicked() {
        stopService(Intent(this, EasyGoService::class.java))
        statusText.text = "⏹ 服务已停止"
        startButton.isEnabled = true
        stopButton.isEnabled  = false
    }
}
