package net.yangkx.mmkv

import android.os.Bundle
import android.util.Log
import androidx.appcompat.app.AppCompatActivity
import net.yangkx.mmkv.demo.databinding.ActivityMainBinding
import kotlin.random.Random

class MainActivity : AppCompatActivity() {
    companion object {
        private const val TAG = "MMKV-APP"
    }

    private lateinit var binding: ActivityMainBinding
    private val mmkv: MMKV
        get() = MyApplication.mmkv

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)
        binding.logView.setContent {
            LogView(leadText = "MMKV Log:")
        }
        binding.string.setOnClickListener {
            val value = mmkv.getString("str_key", "string value") + "1"
            mmkv.putString("str_key", value)
            binding.string.text = value
        }
        binding.integer.setOnClickListener {
            val value = mmkv.getInt("int_key", 0) + 1
            mmkv.putInt("int_key", value)
            binding.integer.text = value.toString()
        }
        binding.bool.setOnClickListener {
            val value = mmkv.getBool("bool_key", false)
            mmkv.putBool("bool_key", !value)
            binding.bool.text = value.toString()
        }
        binding.jlong.setOnClickListener {
            val value = mmkv.getLong("long_key", 0) + 1
            mmkv.putLong("long_key", value)
            binding.jlong.text = value.toString()
        }
        binding.jfloat.setOnClickListener {
            val value = mmkv.getFloat("float_key", 0F) + 1F
            mmkv.putFloat("float_key", value)
            binding.jfloat.text = value.toString()
        }
        binding.jdouble.setOnClickListener {
            val value = mmkv.getDouble("double_key", 0.0) + 1.0
            mmkv.putDouble("double_key", value)
            binding.jdouble.text = value.toString()
        }
        binding.byteArray.setOnClickListener {
            val value = mmkv.getByteArray("byte_array_key", Random.nextBytes(3))
            Random.nextBytes(value)
            mmkv.putByteArray("byte_array_key", value)
            binding.byteArray.text = value.joinToString()
        }
        binding.intArray.setOnClickListener {
            val value = mmkv.getIntArray("int_array_key", intArrayOf(0, 0, 0))
            repeat(value.size) {
                value[it] = Random.nextInt(10)
            }
            mmkv.putIntArray("int_array_key", value)
            binding.intArray.text = value.joinToString()
        }
        binding.longArray.setOnClickListener {
            val value = mmkv.getLongArray("long_array_key", longArrayOf(0, 0, 0))
            repeat(value.size) {
                value[it] = Random.nextLong(10)
            }
            mmkv.putLongArray("long_array_key", value)
            binding.longArray.text = value.joinToString()
        }
        binding.floatArray.setOnClickListener {
            val value = mmkv.getFloatArray("float_array_key", floatArrayOf(0F, 0F, 0F))
            repeat(value.size) {
                value[it] = Random.nextFloat()
            }
            mmkv.putFloatArray("float_array_key", value)
            binding.floatArray.text = value.joinToString()
        }
        binding.doubleArray.setOnClickListener {
            val value = mmkv.getDoubleArray("double_array_key", doubleArrayOf(0.0, 0.0, 0.0))
            repeat(value.size) {
                value[it] = Random.nextDouble(10.0)
            }
            mmkv.putDoubleArray("double_array_key", value)
            binding.doubleArray.text = value.joinToString()
        }
        binding.clearData.setOnClickListener {
            mmkv.clearData()
        }
        try {
            mmkv.getString("not_exists_key")
        } catch (e: KeyNotFoundException) {
            Log.d(TAG, e.message ?: "")
        }
    }
}