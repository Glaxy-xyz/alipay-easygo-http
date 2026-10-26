package com.easygo.http

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import android.util.Log
import androidx.core.app.NotificationCompat
import java.io.File
import java.io.FileOutputStream
import java.util.zip.ZipInputStream

/**
 * 前台服务：解压 APK 内嵌的 Linux armhf 运行时并启动 easygo-http。
 * arm64 包用 qemu-arm-static；arm32 包直接 exec glibc 二进制。
 */
class EasyGoService : Service() {

    companion object {
        private const val TAG = "EasyGoService"
        private const val CHANNEL_ID = "easygo_http_service"
        private const val NOTIFICATION_ID = 1001
        private const val ASSET_ZIP = "easygo-linux.zip"
        private const val RUNTIME_DIR = "easygo-linux"
        private val RUNTIME_VERSION = BuildConfig.RUNTIME_VERSION

        const val EXTRA_ADDR = "addr"
        const val DEFAULT_ADDR = "0.0.0.0:8093"

        private val lifecycleLock = Any()

        @Volatile
        private var serverProcess: Process? = null

        @Volatile
        private var currentAddr: String? = null

        @Volatile
        private var startGeneration = 0
    }

    private var wakeLock: PowerManager.WakeLock? = null

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        val pm = getSystemService(POWER_SERVICE) as PowerManager
        wakeLock = pm.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "easygo:http").apply {
            setReferenceCounted(false)
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val addr = intent?.getStringExtra(EXTRA_ADDR)?.takeIf { it.isNotBlank() } ?: DEFAULT_ADDR
        startForeground(NOTIFICATION_ID, buildNotification(addr, "正在启动…"))
        wakeLock?.acquire()

        val generation = synchronized(lifecycleLock) {
            if (serverProcess?.isAlive == true && currentAddr == addr) {
                Log.i(TAG, "Server already running on $addr")
                return START_STICKY
            }
            currentAddr = addr
            ++startGeneration
        }

        Thread({
            try {
                val runtimeDir = extractRuntime()
                val stale = synchronized(lifecycleLock) {
                    if (generation != startGeneration) {
                        true
                    } else {
                        killLeftoverServers()
                        false
                    }
                }
                if (stale) return@Thread
                Thread.sleep(300)
                if (generation != startGeneration) return@Thread
                startLinuxServer(runtimeDir, addr, generation)
            } catch (e: Exception) {
                Log.e(TAG, "Failed to start server", e)
                if (generation == startGeneration) stopSelf()
            }
        }, "easygo-starter").start()

        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        super.onDestroy()
        synchronized(lifecycleLock) {
            startGeneration++
            currentAddr = null
            killLeftoverServers()
        }
        wakeLock?.let { if (it.isHeld) it.release() }
        stopForeground(STOP_FOREGROUND_REMOVE)
        Log.i(TAG, "Service destroyed")
    }

    /** 退出界面不会带走子进程。按命令行清掉本应用拉起的 easygo-http / qemu。 */
    private fun killLeftoverServers() {
        serverProcess?.destroy()
        serverProcess = null
        val myPid = android.os.Process.myPid()
        File("/proc").list()?.forEach { name ->
            val pid = name.toIntOrNull() ?: return@forEach
            if (pid == myPid) return@forEach
            val cmd = try {
                File("/proc/$pid/cmdline").readBytes()
                    .toString(Charsets.ISO_8859_1)
                    .replace('\u0000', ' ')
            } catch (_: Exception) {
                return@forEach
            }
            val ours = cmd.contains("qemu-arm-static") ||
                cmd.contains("easygo-http") && !cmd.contains("com.easygo.http")
            if (!ours) return@forEach
            Log.i(TAG, "Killing leftover pid=$pid cmd=$cmd")
            try {
                android.os.Process.killProcess(pid)
            } catch (e: Exception) {
                Log.w(TAG, "killProcess $pid failed", e)
            }
        }
    }

    private fun extractRuntime(): File {
        val outDir = File(filesDir, RUNTIME_DIR)
        val marker = File(outDir, ".runtime-version")
        val binaryOk = File(outDir, "easygo-http").canExecute()
        if (binaryOk && marker.exists() && marker.readText().trim() == RUNTIME_VERSION) {
            Log.i(TAG, "Runtime already extracted: $outDir")
            return outDir
        }

        Log.i(TAG, "Extracting $ASSET_ZIP to $outDir")
        outDir.deleteRecursively()
        outDir.mkdirs()

        assets.open(ASSET_ZIP).use { raw ->
            ZipInputStream(raw).use { zip ->
                while (true) {
                    val entry = zip.nextEntry ?: break
                    val dest = File(outDir, entry.name)
                    val destPath = dest.canonicalPath
                    if (!destPath.startsWith(outDir.canonicalPath + File.separator) &&
                        destPath != outDir.canonicalPath
                    ) {
                        throw IllegalStateException("zip path traversal: ${entry.name}")
                    }
                    if (entry.isDirectory) {
                        dest.mkdirs()
                    } else {
                        dest.parentFile?.mkdirs()
                        FileOutputStream(dest).use { zip.copyTo(it) }
                    }
                    zip.closeEntry()
                }
            }
        }

        listOf("qemu-arm-static", "easygo-http", "ld.so").forEach { name ->
            val f = File(outDir, name)
            f.setReadable(true, false)
            f.setExecutable(true, false)
        }
        marker.writeText(RUNTIME_VERSION)
        Log.i(TAG, "Extraction done")
        return outDir
    }

    private fun startLinuxServer(runtimeDir: File, addr: String, generation: Int) {
        val qemu = File(runtimeDir, "qemu-arm-static").absolutePath
        val binary = File(runtimeDir, "easygo-http").absolutePath
        val glibcRoot = File(runtimeDir, "glibc").absolutePath
        val sdkLibs = File(runtimeDir, "sdk/lib").absolutePath
        val glibcLibs = "$glibcRoot/lib/arm-linux-gnueabihf:$glibcRoot/usr/lib/arm-linux-gnueabihf"
        val ldPath = "$sdkLibs:$glibcLibs"

        val useQemu = BuildConfig.USE_QEMU
        val cmd = if (useQemu) {
            listOf(qemu, "-L", glibcRoot, binary)
        } else {
            // 32 位包直接 exec。ld.so 入口已接住 seccomp 的 SIGSYS。
            listOf(
                "/system/bin/sh",
                "-c",
                "GLIBC_TUNABLES=glibc.pthread.rseq=0 exec ./easygo-http",
            )
        }
        Log.i(TAG, "Starting $cmd qemu=$useQemu LD_LIBRARY_PATH=$ldPath addr=$addr")

        val pb = ProcessBuilder(cmd).apply {
            directory(runtimeDir)
            environment().apply {
                keys.filter { it.startsWith("LD_") && it != "LD_LIBRARY_PATH" }.toList()
                    .forEach { remove(it) }
                remove("LD_PRELOAD")
            }
            environment()["LD_LIBRARY_PATH"] = ldPath
            environment()["EASYGO_HTTP_ADDR"] = addr
            environment()["RUST_LOG"] = "info"
            environment()["HOME"] = runtimeDir.absolutePath
            environment()["TMPDIR"] = cacheDir.absolutePath
            // Android 11 seccomp 对 rseq 发 SIGSYS；glibc 2.35+ 默认会注册 rseq。
            environment()["GLIBC_TUNABLES"] = "glibc.pthread.rseq=0"
            redirectErrorStream(true)
        }

        val proc = pb.start()
        synchronized(lifecycleLock) {
            if (generation != startGeneration) {
                proc.destroy()
                return
            }
            serverProcess = proc
        }

        Thread({
            proc.inputStream.bufferedReader().forEachLine { line ->
                Log.i(TAG, line)
            }
        }, "easygo-log").start()

        val code = proc.waitFor()
        val current = synchronized(lifecycleLock) { serverProcess === proc }
        if (current) {
            Log.w(TAG, "easygo-http exited with code $code")
            serverProcess = null
            stopSelf()
        }
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "EasyGo HTTP 服务",
                NotificationManager.IMPORTANCE_LOW,
            ).apply {
                description = "EasyGo POS SDK HTTP 封装服务"
                setShowBadge(false)
            }
            getSystemService(NotificationManager::class.java).createNotificationChannel(channel)
        }
    }

    private fun buildNotification(addr: String, status: String): Notification {
        val pi = PendingIntent.getActivity(
            this,
            0,
            Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("EasyGo HTTP 服务")
            .setContentText("$status  $addr")
            .setSmallIcon(android.R.drawable.ic_menu_share)
            .setContentIntent(pi)
            .setOngoing(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .build()
    }
}
