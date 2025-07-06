#include <stdio.h>
#include <string.h>
#include <assert.h>
#include "../header/vi.h" // Assuming the header is in ../header/

void test_transform_buffer_c() {
    printf("--- Testing transform_buffer_c ---\n");
    char output[100];
    int result;

    // Test VNI
    // The C API processes one word at a time, similar to the Rust library's examples.
    // We need to concatenate results for multi-word tests.
    char combined_output[100];
    combined_output[0] = '\0'; // Start with an empty string

    result = transform_buffer_c("vni", "viet65", output, sizeof(output));
    assert(result == 0);
    strcat(combined_output, output);
    strcat(combined_output, " ");

    result = transform_buffer_c("vni", "nam", output, sizeof(output));
    assert(result == 0);
    strcat(combined_output, output);

    printf("VNI Input: 'viet65 nam', Result: 0 (combined), Output: '%s'\n", combined_output);
    assert(strcmp(combined_output, "việt nam") == 0);

    // Test Telex
    combined_output[0] = '\0';
    result = transform_buffer_c("telex", "vieetj", output, sizeof(output));
    assert(result == 0);
    strcat(combined_output, output);
    strcat(combined_output, " ");

    result = transform_buffer_c("telex", "nam", output, sizeof(output));
    assert(result == 0);
    strcat(combined_output, output);

    printf("Telex Input: 'vieetj nam', Result: 0 (combined), Output: '%s'\n", combined_output);
    assert(strcmp(combined_output, "việt nam") == 0);

    // Test buffer too small
    result = transform_buffer_c("vni", "qua dai qua dai", output, 5);
    printf("VNI Input: 'qua dai qua dai' (small buffer), Result: %d\n", result);
    assert(result != 0); // Should fail

    // Test invalid method
    result = transform_buffer_c("invalid", "test", output, sizeof(output));
    printf("Invalid method Input: 'test', Result: %d\n", result);
    assert(result != 0);
    printf("transform_buffer_c tests PASSED\n\n");
}

void test_incremental_buffer() {
    printf("--- Testing Incremental Buffer ---\n");
    ViIncrementalBuffer* buffer = NULL;
    const char* output_str;

    // Create VNI buffer
    buffer = vi_create_incremental_buffer("vni");
    printf("Created VNI incremental buffer: %p\n", buffer);
    assert(buffer != NULL);

    // Push characters for "việt"
    output_str = vi_incremental_buffer_push(buffer, 'v');
    printf("Push 'v': '%s'\n", output_str);
    assert(strcmp(output_str, "v") == 0);

    output_str = vi_incremental_buffer_push(buffer, 'i');
    printf("Push 'i': '%s'\n", output_str);
    assert(strcmp(output_str, "vi") == 0);

    output_str = vi_incremental_buffer_push(buffer, 'e');
    printf("Push 'e': '%s'\n", output_str);
    assert(strcmp(output_str, "vie") == 0);

    output_str = vi_incremental_buffer_push(buffer, 't');
    printf("Push 't': '%s'\n", output_str);
    assert(strcmp(output_str, "viet") == 0);

    output_str = vi_incremental_buffer_push(buffer, '6');
    printf("Push '6': '%s'\n", output_str);
    // Note: Due to the CString leak, direct pointer comparison is tricky. Content is what matters.
    // This also assumes the leaked pointer is stable for the current test sequence.
    assert(strcmp(output_str, "viêt") == 0);

    output_str = vi_incremental_buffer_push(buffer, '5');
    printf("Push '5': '%s'\n", output_str);
    assert(strcmp(output_str, "việt") == 0);

    const char* final_view = vi_incremental_buffer_view(buffer);
    printf("View: '%s'\n", final_view);
    assert(strcmp(final_view, "việt") == 0);

    const char* input_seq = vi_incremental_buffer_get_input(buffer);
    printf("Input sequence: '%s'\n", input_seq);
    assert(strcmp(input_seq, "viet65") == 0);

    vi_incremental_buffer_clear(buffer);
    printf("Cleared buffer. View after clear: '%s'\n", vi_incremental_buffer_view(buffer));
    assert(strcmp(vi_incremental_buffer_view(buffer), "") == 0);
    assert(strcmp(vi_incremental_buffer_get_input(buffer), "") == 0);

    vi_incremental_buffer_free(buffer);
    printf("Freed buffer.\n");

    // Test with Telex
    buffer = vi_create_incremental_buffer("telex");
    assert(buffer != NULL);
    vi_incremental_buffer_push(buffer, 'n');
    vi_incremental_buffer_push(buffer, 'g');
    vi_incremental_buffer_push(buffer, 'h');
    vi_incremental_buffer_push(buffer, 'i');
    vi_incremental_buffer_push(buffer, 'e');
    vi_incremental_buffer_push(buffer, 'e');
    output_str = vi_incremental_buffer_push(buffer, 'n'); // nghiêng
    printf("Telex 'nghieen': '%s'\n", output_str);
    assert(strcmp(output_str, "nghiên") == 0);
    output_str = vi_incremental_buffer_push(buffer, 'g'); // nghiêng
    printf("Telex 'nghieeng': '%s'\n", output_str);
    assert(strcmp(output_str, "nghiêng") == 0);
    vi_incremental_buffer_free(buffer);

    printf("Incremental buffer tests PASSED\n\n");
}

void test_transform_buffer_with_style() {
    printf("--- Testing transform_buffer_with_style ---\n");
    char output[100];
    int result;

    // Test VNI Old Style: hòa (o + grave) vs hoà (a + grave)
    // For VNI, "hoa2" -> "hòa" (new) vs "hoà" (old)
    // The library's default "VNI" might already be new style.
    // Let's test "hoaf" with Telex old vs new.
    // Old: "hòa" (grave on a)
    // New: "hoà" (grave on o) - this lib's default is new style accent placement.
    // The example was: `hoà` instead of `hòa` (new style)
    // So, Old style should give "hòa"
    result = vi_transform_buffer_with_style("telex", Old, "hoaf", output, sizeof(output));
    printf("Telex OLD Style Input: 'hoaf', Result: %d, Output: '%s'\n", result, output);
    assert(result == 0 && strcmp(output, "hòa") == 0);

    result = vi_transform_buffer_with_style("telex", New, "hoaf", output, sizeof(output));
    printf("Telex NEW Style Input: 'hoaf', Result: %d, Output: '%s'\n", result, output);
    assert(result == 0 && strcmp(output, "hoà") == 0);

    // Test VNI style
    result = vi_transform_buffer_with_style("vni", Old, "hoa2", output, sizeof(output));
    printf("VNI OLD Style Input: 'hoa2', Result: %d, Output: '%s'\n", result, output);
    assert(result == 0 && strcmp(output, "hòa") == 0);

    result = vi_transform_buffer_with_style("vni", New, "hoa2", output, sizeof(output));
    printf("VNI NEW Style Input: 'hoa2', Result: %d, Output: '%s'\n", result, output);
    assert(result == 0 && strcmp(output, "hoà") == 0);


    printf("transform_buffer_with_style tests PASSED\n\n");
}


int main() {
    printf("Starting C API tests...\n");
    test_transform_buffer_c();
    test_incremental_buffer();
    test_transform_buffer_with_style();
    printf("All C API tests completed successfully!\n");
    return 0;
}
