package net.yangkx.mmkv

import androidx.appcompat.app.AppCompatActivity
import android.os.Bundle
import net.yangkx.mmkv.databinding.ActivityMainBinding

class MainActivity : AppCompatActivity() {

    private lateinit var binding: ActivityMainBinding

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)
        MMKV.putString("first_key", "first value")
        MMKV.putInt("second_key", 1024)
        MMKV.putBool("third_key", true)
        binding.string.text = MMKV.getString("first_key")
        binding.integer.text = MMKV.getInt("second_key").toString()
        binding.bool.text = MMKV.getBool("third_key").toString()
        MMKV.clearData()
//        MMKV.close()
    }
}