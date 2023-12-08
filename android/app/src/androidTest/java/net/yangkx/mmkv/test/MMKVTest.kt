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
    }

    @After
    fun clean() {
        MMKV.clearData()
    }

    private fun reInitSDK() {
        MMKV.close()
        MMKVInitializer.init(appContext)
    }

    @Test
    fun testStringAndBoolApi() {
        MMKV.putString("str_key", "test_str_value")
        MMKV.putBool("bool_key", true)
        reInitSDK()
        assertEquals(MMKV.getString("str_key"), "test_str_value")
        assertEquals(MMKV.getBool("bool_key"), true)
        MMKV.putBool("bool_key", false)
        reInitSDK()
        assertEquals(MMKV.getBool("bool_key"), false)
    }

    @Test
    fun testIntApi() {
        val random = Random.nextInt()
        MMKV.putInt("int_random_key", random)
        MMKV.putInt("int_max_key", Int.MAX_VALUE)
        MMKV.putInt("int_min_key", Int.MIN_VALUE)
        reInitSDK()
        assertEquals(MMKV.getInt("int_random_key"), random)
        assertEquals(MMKV.getInt("int_max_key"), Int.MAX_VALUE)
        assertEquals(MMKV.getInt("int_min_key"), Int.MIN_VALUE)
    }

    @Test
    fun testLongApi() {
        val random = Random.nextLong()
        MMKV.putLong("long_random_key", random)
        MMKV.putLong("long_max_key", Long.MAX_VALUE)
        MMKV.putLong("long_min_key", Long.MIN_VALUE)
        reInitSDK()
        assertEquals(MMKV.getLong("long_random_key"), random)
        assertEquals(MMKV.getLong("long_max_key"), Long.MAX_VALUE)
        assertEquals(MMKV.getLong("long_min_key"), Long.MIN_VALUE)
    }

    @Test
    fun testFloatApi() {
        val random = Random.nextFloat()
        MMKV.putFloat("float_random_key", random)
        MMKV.putFloat("float_max_key", Float.MAX_VALUE)
        MMKV.putFloat("float_min_key", Float.MIN_VALUE)
        reInitSDK()
        assertEquals(MMKV.getFloat("float_random_key"), random)
        assertEquals(MMKV.getFloat("float_max_key"), Float.MAX_VALUE, 0f)
        assertEquals(MMKV.getFloat("float_min_key"), Float.MIN_VALUE, 0f)
    }

    @Test
    fun testDoubleApi() {
        val random = Random.nextDouble()
        MMKV.putDouble("double_random_key", random)
        MMKV.putDouble("double_max_key", Double.MAX_VALUE)
        MMKV.putDouble("double_min_key", Double.MIN_VALUE)
        reInitSDK()
        assertEquals(MMKV.getDouble("double_random_key"), random, 0.0)
        assertEquals(MMKV.getDouble("double_max_key"), Double.MAX_VALUE, 0.0)
        assertEquals(MMKV.getDouble("double_min_key"), Double.MIN_VALUE, 0.0)
    }

    @Test
    fun testByteArrayApi() {
        val random = Random.nextBytes(1)[0]
        val array = byteArrayOf(Byte.MIN_VALUE, random, Byte.MAX_VALUE)
        MMKV.putByteArray("byte_array_key", array)
        reInitSDK()
        assertArrayEquals(MMKV.getByteArray("byte_array_key"), array)
    }

    @Test
    fun testIntArrayApi() {
        val random = Random.nextInt()
        val array = intArrayOf(Int.MIN_VALUE, random, Int.MAX_VALUE)
        MMKV.putIntArray("int_array_key", array)
        reInitSDK()
        assertArrayEquals(MMKV.getIntArray("int_array_key"), array)
    }

    @Test
    fun testLongArrayApi() {
        val random = Random.nextLong()
        val array = longArrayOf(Long.MIN_VALUE, random, Long.MAX_VALUE)
        MMKV.putLongArray("long_array_key", array)
        reInitSDK()
        assertArrayEquals(MMKV.getLongArray("long_array_key"), array)
    }

    @Test
    fun testFloatArrayApi() {
        val random = Random.nextFloat()
        val array = floatArrayOf(Float.MIN_VALUE, random, Float.MAX_VALUE)
        MMKV.putFloatArray("float_array_key", array)
        reInitSDK()
        assertArrayEquals(MMKV.getFloatArray("float_array_key"), array, 0f)
    }

    @Test
    fun testDoubleArrayApi() {
        val random = Random.nextDouble()
        val array = doubleArrayOf(Double.MIN_VALUE, random, Double.MAX_VALUE)
        MMKV.putDoubleArray("double_array_key", array)
        reInitSDK()
        assertArrayEquals(MMKV.getDoubleArray("double_array_key"), array, 0.0)
    }

    @Test
    fun testMultiThread() {
        MMKV.clearData()
        MMKVInitializer.init(appContext)
        MMKV.setLogLevel(LogLevel.INFO)
        val threadArray = mutableListOf<Thread>()
        Thread {
            var strValue = ""
            val key = "multi_thread_str_key"
            repeat(100) {
                strValue += it.toString()
                MMKV.putString(key, strValue)
            }
        }.apply {
            threadArray.add(this)
            start()
        }
        repeat(2) { i ->
            Thread {
                repeat(500) {
                    MMKV.putInt("task_${i}_key_$it", it)
                }
            }.apply {
                threadArray.add(this)
                start()
            }
        }
        threadArray.forEach { it.join() }
        reInitSDK()
        threadArray.clear()

        MMKV.setLogLevel(LogLevel.INFO)
        repeat(2) { i ->
            Thread {
                repeat(500) {
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