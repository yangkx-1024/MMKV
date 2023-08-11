package net.yangkx.mmkv

import android.annotation.SuppressLint
import android.os.Bundle
import android.text.method.ScrollingMovementMethod
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import net.yangkx.mmkv.databinding.ActivityMainBinding
import net.yangkx.mmkv.log.LogLevel
import net.yangkx.mmkv.log.Logger

class MainActivity : AppCompatActivity() {
    companion object {
        private const val TAG = "MMKV-APP"
    }

    private lateinit var binding: ActivityMainBinding
    private val logger = object : Logger {
        override fun verbose(log: String) {
            Log.v(TAG, log)
            appendLog("V - $log")
        }

        override fun info(log: String) {
            Log.i(TAG, log)
            appendLog("I - $log")
        }

        override fun debug(log: String) {
            Log.d(TAG, log)
            appendLog("D - $log")
        }

        override fun warn(log: String) {
            Log.w(TAG, log)
            appendLog("W - $log")
        }

        override fun error(log: String) {
            Log.e(TAG, log)
            appendLog("E - $log")
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)
        binding.log.movementMethod = ScrollingMovementMethod.getInstance()
        appendLog("Test logs:")
        MMKV.setLogger(logger)
        MMKV.setLogLevel(LogLevel.VERBOSE)
        binding.string.setOnClickListener {
            val value = MMKV.getString("str_key", "string value") + "1"
            MMKV.putString("str_key", value)
            binding.string.text = value
        }
        binding.integer.setOnClickListener {
            val value = MMKV.getInt("int_key", 0) + 1
            MMKV.putInt("int_key", value)
            binding.integer.text = value.toString()
        }
        binding.bool.setOnClickListener {
            val value = MMKV.getBool("bool_key", false)
            MMKV.putBool("bool_key", !value)
            binding.bool.text = value.toString()
        }
        binding.string.text = MMKV.getString("str_key", "string value")
        binding.integer.text = MMKV.getInt("int_key", 0).toString()
        binding.bool.text = MMKV.getBool("bool_key", false).toString()
        try {
            MMKV.getString("not_exists_key")
        } catch (e: KeyNotFoundException) {
            Log.d(TAG, e.message ?: "")
        }
    }

    @SuppressLint("SetTextI18n")
    private fun appendLog(log: String) {
        binding.log.text = binding.log.text.toString() + "\n" + log
    }
}