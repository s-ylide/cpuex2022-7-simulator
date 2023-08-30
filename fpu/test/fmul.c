#include <stdio.h>
#include <math.h>
#include <stdlib.h>
#include "../impl/fpulib.h"
#include "../impl/fmul.h"

// 実装基準を満たすかのテスト用
void test_fmul(void) {
  uint32_t m1, m2;
  union Num x1_num, x2_num, y_true, y_num;
  for (int32_t i = 1; i < 254; ++i) {
    for (int32_t j = 1; j < 254; ++j) {
      for (int32_t s1 = 0; s1 < 2; ++s1) {
        for (int32_t s2 = 0; s2 < 2; ++s2) {
          for (int32_t it = 0; it < 10; ++it) {
            for (int32_t jt = 0; jt < 10; ++jt) {
              switch (it) {
                case 0 : m1 = 0; break;
                case 1 : m1 = 1; break;
                case 2 : m1 = 2; break;
                case 3 : m1 = 0x380000; break;
                case 4 : m1 = 0x400000; break;
                case 5 : m1 = 0x5fffff; break;
                case 6 : m1 = 0x7fffff; break;
                default : m1 = slice(rand(), 23, 1); break;
              }
              switch (jt) {
                case 0 : m2 = 0; break;
                case 1 : m2 = 1; break;
                case 2 : m2 = 2; break;
                case 3 : m2 = 0x380000; break;
                case 4 : m2 = 0x400000; break;
                case 5 : m2 = 0x5fffff; break;
                case 6 : m2 = 0x7fffff; break;
                default : m2 = slice(rand(), 23, 1); break;
              }
              x1_num.nat = mkfloat(s1, i, m1);
              x2_num.nat = mkfloat(s2, j, m2);
              y_true.real = x1_num.real * x2_num.real;
              y_num.real = fmul(x1_num.real, x2_num.real);
              if (fabs(y_num.real-y_true.real) >= fabs(y_true.real)*pow(2,-22)
                  && fabs(y_num.real-y_true.real) >= pow(2,-126)
                  && slice(y_true.nat, 31, 24) != 0
                  && slice(y_true.nat, 31, 24) != 255
                  && slice(y_true.nat, 31, 24) != 254) {
                printf("%f %f %f %f\n", x1_num.real, x2_num.real, y_true.real, y_num.real);
                printf("%u %u\n", x1_num.nat, x2_num.nat);
                printf("%u\n", y_num.nat);
                puts("here");
              }
            }
          }
        }
      }
    }
  }
}

// Verilogの方と出力が一致するかのテスト用
void test_fmul_emu(void) {
  FILE *fp;
  fp = fopen("fmul_emu.txt", "w");
  uint32_t m1, m2;
  union Num x1_num, x2_num, y_num;
  for (int32_t i = 1; i < 254; i += 7) {
    for (int32_t j = 1; j < 254; j+= 7) {
      for (int32_t s1 = 0; s1 < 2; ++s1) {
        for (int32_t s2 = 0; s2 < 2; ++s2) {
          m1 = slice(rand(), 23, 1);
          m2 = slice(rand(), 23, 1);
          x1_num.nat = mkfloat(s1, i, m1);
          x2_num.nat = mkfloat(s2, j, m2);
          y_num.real = fmul(x1_num.real, x2_num.real);
          for (int32_t k = 0; k < 32; k++) {
            int32_t bin = 1 & ((x1_num.nat)>>(31-k));
            fprintf(fp, "%d", bin);
          }
          fprintf(fp, "\n");
          for (int32_t k = 0; k < 32; k++) {
            int32_t bin = 1 & ((x2_num.nat)>>(31-k));
            fprintf(fp, "%d", bin);
          }
          fprintf(fp, "\n");
          for (int32_t k = 0; k < 32; k++) {
            int32_t bin = 1 & ((y_num.nat)>>(31-k));
            fprintf(fp, "%d", bin);
          }
          fprintf(fp, "\n");
        }
      }
    }
  }
  fclose(fp);
}
