import sys
nums = sorted(int(x) for x in sys.stdin.read().split())
mid = len(nums) // 2
print(nums[mid] if len(nums) % 2 else (nums[mid - 1] + nums[mid]) / 2)
