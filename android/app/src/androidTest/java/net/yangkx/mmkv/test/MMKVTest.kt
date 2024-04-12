package net.yangkx.mmkv.test

import android.content.Context
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.filters.LargeTest
import androidx.test.platform.app.InstrumentationRegistry
import net.yangkx.mmkv.MMKV
import net.yangkx.mmkv.MMKVInitializer
import net.yangkx.mmkv.log.LogLevel
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import kotlin.random.Random

@RunWith(AndroidJUnit4::class)
@LargeTest
class MMKVTest {
    private val appContext: Context
        get() = InstrumentationRegistry.getInstrumentation().targetContext
    private var mmkv: MMKV? = null

    @Before
    fun setUp() {
        mmkv = MMKVInitializer.init(appContext)
        mmkv!!.clearData()
    }

    @After
    fun clean() {
        mmkv?.clearData()
    }

    private fun initSdk() {
        mmkv = null
        System.runFinalization()
        mmkv = MMKVInitializer.init(appContext)
    }

    @Test
    fun testStringAndBoolApi() {
        initSdk()
        mmkv!!.putString("str_key", "test_str_value")
        mmkv!!.putBool("bool_key", true)
        initSdk()
        assertEquals(mmkv!!.getString("str_key"), "test_str_value")
        assertEquals(mmkv!!.getBool("bool_key"), true)
        mmkv!!.putBool("bool_key", false)
        initSdk()
        assertEquals(mmkv!!.getBool("bool_key"), false)
    }

    @Test
    fun testIntApi() {
        initSdk()
        val random = Random.nextInt()
        mmkv!!.putInt("int_random_key", random)
        mmkv!!.putInt("int_max_key", Int.MAX_VALUE)
        mmkv!!.putInt("int_min_key", Int.MIN_VALUE)
        initSdk()
        assertEquals(mmkv!!.getInt("int_random_key"), random)
        assertEquals(mmkv!!.getInt("int_max_key"), Int.MAX_VALUE)
        assertEquals(mmkv!!.getInt("int_min_key"), Int.MIN_VALUE)
    }

    @Test
    fun testLongApi() {
        initSdk()
        val random = Random.nextLong()
        mmkv!!.putLong("long_random_key", random)
        mmkv!!.putLong("long_max_key", Long.MAX_VALUE)
        mmkv!!.putLong("long_min_key", Long.MIN_VALUE)
        initSdk()
        assertEquals(mmkv!!.getLong("long_random_key"), random)
        assertEquals(mmkv!!.getLong("long_max_key"), Long.MAX_VALUE)
        assertEquals(mmkv!!.getLong("long_min_key"), Long.MIN_VALUE)
    }

    @Test
    fun testFloatApi() {
        initSdk()
        val random = Random.nextFloat()
        mmkv!!.putFloat("float_random_key", random)
        mmkv!!.putFloat("float_max_key", Float.MAX_VALUE)
        mmkv!!.putFloat("float_min_key", Float.MIN_VALUE)
        initSdk()
        assertEquals(mmkv!!.getFloat("float_random_key"), random)
        assertEquals(mmkv!!.getFloat("float_max_key"), Float.MAX_VALUE, 0f)
        assertEquals(mmkv!!.getFloat("float_min_key"), Float.MIN_VALUE, 0f)
    }

    @Test
    fun testDoubleApi() {
        initSdk()
        val random = Random.nextDouble()
        mmkv!!.putDouble("double_random_key", random)
        mmkv!!.putDouble("double_max_key", Double.MAX_VALUE)
        mmkv!!.putDouble("double_min_key", Double.MIN_VALUE)
        initSdk()
        assertEquals(mmkv!!.getDouble("double_random_key"), random, 0.0)
        assertEquals(mmkv!!.getDouble("double_max_key"), Double.MAX_VALUE, 0.0)
        assertEquals(mmkv!!.getDouble("double_min_key"), Double.MIN_VALUE, 0.0)
    }

    @Test
    fun testByteArrayApi() {
        initSdk()
        val random = Random.nextBytes(1)[0]
        val array = byteArrayOf(Byte.MIN_VALUE, random, Byte.MAX_VALUE)
        mmkv!!.putByteArray("byte_array_key", array)
        initSdk()
        assertArrayEquals(mmkv!!.getByteArray("byte_array_key"), array)
    }

    @Test
    fun testIntArrayApi() {
        initSdk()
        val random = Random.nextInt()
        val array = intArrayOf(Int.MIN_VALUE, random, Int.MAX_VALUE)
        mmkv!!.putIntArray("int_array_key", array)
        initSdk()
        assertArrayEquals(mmkv!!.getIntArray("int_array_key"), array)
    }

    @Test
    fun testLongArrayApi() {
        initSdk()
        val random = Random.nextLong()
        val array = longArrayOf(Long.MIN_VALUE, random, Long.MAX_VALUE)
        mmkv!!.putLongArray("long_array_key", array)
        initSdk()
        assertArrayEquals(mmkv!!.getLongArray("long_array_key"), array)
    }

    @Test
    fun testFloatArrayApi() {
        initSdk()
        val random = Random.nextFloat()
        val array = floatArrayOf(Float.MIN_VALUE, random, Float.MAX_VALUE)
        mmkv!!.putFloatArray("float_array_key", array)
        initSdk()
        assertArrayEquals(mmkv!!.getFloatArray("float_array_key"), array, 0f)
    }

    @Test
    fun testDoubleArrayApi() {
        initSdk()
        val random = Random.nextDouble()
        val array = doubleArrayOf(Double.MIN_VALUE, random, Double.MAX_VALUE)
        mmkv!!.putDoubleArray("double_array_key", array)
        initSdk()
        assertArrayEquals(mmkv!!.getDoubleArray("double_array_key"), array, 0.0)
    }

    @Test
    fun testPutAndDelete() {
        val key = "key_to_delete"
        initSdk()
        mmkv!!.putInt(key, 1)
        initSdk()
        mmkv!!.delete(key)
        assertEquals(mmkv!!.getInt(key, -1), -1)
        initSdk()
        assertEquals(mmkv!!.getInt(key, -1), -1)
    }

    @Test
    fun testMultiThread() {
        clean()
        MMKV.setLogLevel(LogLevel.VERBOSE)
        val threadArray = mutableListOf<Thread>()
        val repeatCount = 1000
        Thread {
            val key = "multi_thread_repeat_key"
            repeat(repeatCount) {
                if (it % 2 == 0) {
                    mmkv!!.putInt(key, it)
                } else {
                    mmkv!!.delete(key)
                }
            }
        }.apply {
            threadArray.add(this)
            start()
        }
        repeat(2) { i ->
            Thread {
                repeat(repeatCount) {
                    mmkv!!.putInt("task_${i}_key_$it", it)
                }
            }.apply {
                threadArray.add(this)
                start()
            }
        }
        threadArray.forEach { it.join() }
        initSdk()
        threadArray.clear()

        MMKV.setLogLevel(LogLevel.INFO)
        assertEquals(mmkv!!.getInt("multi_thread_repeat_key", -1), -1)
        repeat(2) { i ->
            Thread {
                repeat(repeatCount) {
                    assertEquals(mmkv!!.getInt("task_${i}_key_$it"), it)
                }
            }.apply {
                threadArray.add(this)
                start()
            }
        }
        threadArray.forEach { it.join() }
    }
}