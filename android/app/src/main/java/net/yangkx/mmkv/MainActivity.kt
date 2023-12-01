package net.yangkx.mmkv

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import net.yangkx.mmkv.demo.databinding.ActivityMainBinding
import net.yangkx.mmkv.log.LogLevel
import kotlin.random.Random

class MainActivity : AppCompatActivity() {
    companion object {
        private const val TAG = "MMKV-APP"
    }

    private lateinit var binding: ActivityMainBinding

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)
        binding.logView.setContent {
            LogView(leadText = "MMKV Log:", logLevel = LogLevel.VERBOSE)
        }
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
        binding.jlong.setOnClickListener {
            val value = MMKV.getLong("long_key", 0) + 1
            MMKV.putLong("long_key", value)
            binding.jlong.text = value.toString()
        }
        binding.jfloat.setOnClickListener {
            val value = MMKV.getFloat("float_key", 0F) + 1F
            MMKV.putFloat("float_key", value)
            binding.jfloat.text = value.toString()
        }
        binding.jdouble.setOnClickListener {
            val value = MMKV.getDouble("double_key", 0.0) + 1.0
            MMKV.putDouble("double_key", value)
            binding.jdouble.text = value.toString()
        }
        binding.byteArray.setOnClickListener {
            val value = MMKV.getByteArray("byte_array_key", Random.nextBytes(3))
            Random.nextBytes(value)
            MMKV.putByteArray("byte_array_key", value)
            binding.byteArray.text = value.joinToString()
        }
        binding.intArray.setOnClickListener {
            val value = MMKV.getIntArray("int_array_key", intArrayOf(0, 0, 0))
            repeat(value.size) {
                value[it] = Random.nextInt(10)
            }
            MMKV.putIntArray("int_array_key", value)
            binding.intArray.text = value.joinToString()
        }
        binding.longArray.setOnClickListener {
            val value = MMKV.getLongArray("long_array_key", longArrayOf(0, 0, 0))
            repeat(value.size) {
                value[it] = Random.nextLong(10)
            }
            MMKV.putLongArray("long_array_key", value)
            binding.longArray.text = value.joinToString()
        }
        binding.floatArray.setOnClickListener {
            val value = MMKV.getFloatArray("float_array_key", floatArrayOf(0F, 0F, 0F))
            repeat(value.size) {
                value[it] = Random.nextFloat()
            }
            MMKV.putFloatArray("float_array_key", value)
            binding.floatArray.text = value.joinToString()
        }
        binding.doubleArray.setOnClickListener {
            val value = MMKV.getDoubleArray("double_array_key", doubleArrayOf(0.0, 0.0, 0.0))
            repeat(value.size) {
                value[it] = Random.nextDouble(10.0)
            }
            MMKV.putDoubleArray("double_array_key", value)
            binding.doubleArray.text = value.joinToString()
        }
        binding.clearData.setOnClickListener {
            MMKV.clearData()
            MMKVInitializer.init(this@MainActivity)
        }
        try {
            MMKV.getString("not_exists_key")
        } catch (e: KeyNotFoundException) {
            Log.d(TAG, e.message ?: "")
        }
    }
}