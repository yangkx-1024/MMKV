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

    @Before
    fun setUp() {
        MMKVInitializer.init(appContext)
        MMKV.clearData()
    }

    @After
    fun clean() {
        MMKVInitializer.init(appContext)
        MMKV.clearData()
    }

    private fun initSdk() {
        MMKVInitializer.init(appContext)
    }

    @Test
    fun testStringAndBoolApi() {
        initSdk()
        MMKV.putString("str_key", "test_str_value")
        MMKV.putBool("bool_key", true)
        initSdk()
        assertEquals(MMKV.getString("str_key"), "test_str_value")
        assertEquals(MMKV.getBool("bool_key"), true)
        MMKV.putBool("bool_key", false)
        initSdk()
        assertEquals(MMKV.getBool("bool_key"), false)
    }

    @Test
    fun testIntApi() {
        initSdk()
        val random = Random.nextInt()
        MMKV.putInt("int_random_key", random)
        MMKV.putInt("int_max_key", Int.MAX_VALUE)
        MMKV.putInt("int_min_key", Int.MIN_VALUE)
        initSdk()
        assertEquals(MMKV.getInt("int_random_key"), random)
        assertEquals(MMKV.getInt("int_max_key"), Int.MAX_VALUE)
        assertEquals(MMKV.getInt("int_min_key"), Int.MIN_VALUE)
    }

    @Test
    fun testLongApi() {
        initSdk()
        val random = Random.nextLong()
        MMKV.putLong("long_random_key", random)
        MMKV.putLong("long_max_key", Long.MAX_VALUE)
        MMKV.putLong("long_min_key", Long.MIN_VALUE)
        initSdk()
        assertEquals(MMKV.getLong("long_random_key"), random)
        assertEquals(MMKV.getLong("long_max_key"), Long.MAX_VALUE)
        assertEquals(MMKV.getLong("long_min_key"), Long.MIN_VALUE)
    }

    @Test
    fun testFloatApi() {
        initSdk()
        val random = Random.nextFloat()
        MMKV.putFloat("float_random_key", random)
        MMKV.putFloat("float_max_key", Float.MAX_VALUE)
        MMKV.putFloat("float_min_key", Float.MIN_VALUE)
        initSdk()
        assertEquals(MMKV.getFloat("float_random_key"), random)
        assertEquals(MMKV.getFloat("float_max_key"), Float.MAX_VALUE, 0f)
        assertEquals(MMKV.getFloat("float_min_key"), Float.MIN_VALUE, 0f)
    }

    @Test
    fun testDoubleApi() {
        initSdk()
        val random = Random.nextDouble()
        MMKV.putDouble("double_random_key", random)
        MMKV.putDouble("double_max_key", Double.MAX_VALUE)
        MMKV.putDouble("double_min_key", Double.MIN_VALUE)
        initSdk()
        assertEquals(MMKV.getDouble("double_random_key"), random, 0.0)
        assertEquals(MMKV.getDouble("double_max_key"), Double.MAX_VALUE, 0.0)
        assertEquals(MMKV.getDouble("double_min_key"), Double.MIN_VALUE, 0.0)
    }

    @Test
    fun testByteArrayApi() {
        initSdk()
        val random = Random.nextBytes(1)[0]
        val array = byteArrayOf(Byte.MIN_VALUE, random, Byte.MAX_VALUE)
        MMKV.putByteArray("byte_array_key", array)
        initSdk()
        assertArrayEquals(MMKV.getByteArray("byte_array_key"), array)
    }

    @Test
    fun testIntArrayApi() {
        initSdk()
        val random = Random.nextInt()
        val array = intArrayOf(Int.MIN_VALUE, random, Int.MAX_VALUE)
        MMKV.putIntArray("int_array_key", array)
        initSdk()
        assertArrayEquals(MMKV.getIntArray("int_array_key"), array)
    }

    @Test
    fun testLongArrayApi() {
        initSdk()
        val random = Random.nextLong()
        val array = longArrayOf(Long.MIN_VALUE, random, Long.MAX_VALUE)
        MMKV.putLongArray("long_array_key", array)
        initSdk()
        assertArrayEquals(MMKV.getLongArray("long_array_key"), array)
    }

    @Test
    fun testFloatArrayApi() {
        initSdk()
        val random = Random.nextFloat()
        val array = floatArrayOf(Float.MIN_VALUE, random, Float.MAX_VALUE)
        MMKV.putFloatArray("float_array_key", array)
        initSdk()
        assertArrayEquals(MMKV.getFloatArray("float_array_key"), array, 0f)
    }

    @Test
    fun testDoubleArrayApi() {
        initSdk()
        val random = Random.nextDouble()
        val array = doubleArrayOf(Double.MIN_VALUE, random, Double.MAX_VALUE)
        MMKV.putDoubleArray("double_array_key", array)
        initSdk()
        assertArrayEquals(MMKV.getDoubleArray("double_array_key"), array, 0.0)
    }

    @Test
    fun testPutAndDelete() {
        val key = "key_to_delete"
        initSdk()
        MMKV.putInt(key, 1)
        initSdk()
        MMKV.delete(key)
        assertEquals(MMKV.getInt(key, -1), -1)
        initSdk()
        assertEquals(MMKV.getInt(key, -1), -1)
    }

    @Test
    fun testMultiThread() {
        clean()
        initSdk()
        MMKV.setLogLevel(LogLevel.VERBOSE)
        val threadArray = mutableListOf<Thread>()
        val repeatCount = 1000
        Thread {
            val key = "multi_thread_repeat_key"
            repeat(repeatCount) {
                if (it % 2 == 0) {
                    MMKV.putInt(key, it)
                } else {
                    MMKV.delete(key)
                }
            }
        }.apply {
            threadArray.add(this)
            start()
        }
        repeat(2) { i ->
            Thread {
                repeat(repeatCount) {
                    MMKV.putInt("task_${i}_key_$it", it)
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
        assertEquals(MMKV.getInt("multi_thread_repeat_key", -1), -1)
        repeat(2) { i ->
            Thread {
                repeat(repeatCount) {
                    assertEquals(MMKV.getInt("task_${i}_key_$it"), it)
                }
            }.apply {
                threadArray.add(this)
                start()
            }
        }
        threadArray.forEach { it.join() }
    }
}