// Profile 14 reference-only observer. Included as exact bytes by the source adapter.
// The actual pinned VM owns every element value and executes every operation.
// This ledger records only origins, kind, length, lifetime and pending stack shape;
// it never evaluates element writes/reads or traverses heap/aggregate graphs.
class indexed_storage_observer {
    struct allocation {
        bool array;
        bool text;
        bool released;
        std::size_t length;
        std::int64_t address;
    };
    enum class observation_kind { allocation, integer, text, mutation };
    struct observation {
        observation_kind kind;
        std::size_t stack_size;
        std::optional<std::size_t> allocation_index;
        std::optional<std::int64_t> expected_integer;
        bool empty_text;
    };

public:
    void observed(const ny::VM &running) {
        if (!pending_) {
            return;
        }
        const auto &stack = running.getStack();
        if (stack.size() != pending_->stack_size) {
            refuse("executed stack shape drift");
        }
        if (pending_->kind == observation_kind::allocation) {
            auto &entry = allocations_[*pending_->allocation_index];
            const auto *array = stack.empty() ? nullptr : std::get_if<ny::ArrayHandle>(&stack.back());
            const auto *integer = stack.empty() ? nullptr : std::get_if<std::int64_t>(&stack.back());
            if ((entry.array && !array) || (!entry.array && !integer)) {
                refuse("executed allocation kind drift");
            }
            const auto address = entry.array ? array->address : *integer;
            // Addresses are observed from the VM, never manufactured. With only
            // guarded literal/ALLOC producers they must be fresh aligned regions.
            const auto span = static_cast<std::uint64_t>(std::max<std::size_t>(entry.length, 1)) * 8;
            if (address < 0 || address % 8 != 0 || identities_.count(address) != 0 ||
                static_cast<std::uint64_t>(address) < next_minimum_address_ ||
                static_cast<std::uint64_t>(address) >
                    static_cast<std::uint64_t>(std::numeric_limits<std::int64_t>::max()) - span) {
                refuse("executed allocation address drift");
            }
            entry.address = address;
            identities_.emplace(address, *pending_->allocation_index);
            next_minimum_address_ = static_cast<std::uint64_t>(address) + span;
        } else if (pending_->kind == observation_kind::integer) {
            const auto *integer = stack.empty() ? nullptr : std::get_if<std::int64_t>(&stack.back());
            if (!integer || (pending_->expected_integer && *integer != *pending_->expected_integer)) {
                refuse("executed integer observation drift");
            }
        } else if (pending_->kind == observation_kind::text) {
            const auto *text = stack.empty() ? nullptr : std::get_if<std::string>(&stack.back());
            if (!text || (pending_->empty_text && !text->empty())) {
                refuse("executed text observation drift");
            }
        }
        pending_.reset();
    }

    void check_array(const ny::Value &value) const {
        if (const auto *array = std::get_if<ny::ArrayHandle>(&value)) {
            const auto found = identities_.find(array->address);
            if (found == identities_.end() || !allocations_[found->second].array) {
                refuse("unregistered array handle");
            }
        }
    }

    void require_buffer(const ny::Value &value, std::size_t identity) const {
        const auto *address = std::get_if<std::int64_t>(&value);
        if (identity >= allocations_.size() || allocations_[identity].array ||
            !address || *address != allocations_[identity].address) {
            refuse("buffer origin/address drift");
        }
    }

    std::size_t allocate(const std::vector<ny::Value> &stack) {
        if (stack.empty()) {
            refuse("ALLOC requires explicit integer count");
        }
        const auto count = integer(stack.back());
        const auto length = count < 1 ? std::size_t(1) : static_cast<std::uint64_t>(count);
        return reserve(false, false, length, stack.size());
    }

    void literal(const std::vector<ny::Value> &stack, std::size_t count, bool text) {
        if (count > stack.size()) {
            refuse("literal stack underflow");
        }
        reserve(true, text, count, stack.size() - count + 1);
    }

    void get(const std::vector<ny::Value> &stack, std::optional<std::size_t> buffer) {
        if (stack.size() < 2) {
            refuse("GET_INDEX operand underflow");
        }
        const auto index = integer(stack.back());
        const auto &entry = receiver(stack[stack.size() - 2], buffer);
        const bool zero = entry.released || index < 0 || static_cast<std::uint64_t>(index) >= entry.length;
        pending_ = observation{entry.text ? observation_kind::text : observation_kind::integer,
            stack.size() - 1, std::nullopt,
            zero && !entry.text ? std::optional<std::int64_t>(0) : std::nullopt, zero && entry.text};
    }

    void set(const std::vector<ny::Value> &stack, std::optional<std::size_t> buffer) {
        if (stack.size() < 3) {
            refuse("SET_INDEX operand underflow");
        }
        integer(stack.back());
        integer(stack[stack.size() - 2]);
        const auto &entry = receiver(stack[stack.size() - 3], buffer);
        if (entry.text) {
            refuse("mutation requires integer storage");
        }
        mutation(stack.size() - 3);
    }

    void release(const std::vector<ny::Value> &stack, std::optional<std::size_t> buffer) {
        if (stack.empty() || !buffer) {
            refuse("FREE requires buffer origin");
        }
        require_buffer(stack.back(), *buffer);
        auto &entry = allocations_[*buffer];
        if (entry.released) {
            // The real VM writes stderr for a second FREE. Keep that diagnostic
            // outside this quiet reference protocol; candidate-only tests cover it.
            refuse("repeated FREE outside quiet reference policy");
        }
        entry.released = true;
        mutation(stack.size() - 1);
    }

    void append(const std::vector<ny::Value> &stack) {
        if (stack.size() < 2) {
            refuse("array__push operand underflow");
        }
        integer(stack.back());
        auto &entry = array_entry(stack[stack.size() - 2]);
        if (entry.text) {
            refuse("mutation requires integer storage");
        }
        charge(1);
        ++entry.length;
        mutation(stack.size() - 2);
    }

    void length(const std::vector<ny::Value> &stack) {
        if (stack.empty()) {
            refuse("LEN operand underflow");
        }
        const auto &entry = array_entry(stack.back());
        pending_ = observation{observation_kind::integer, stack.size(), std::nullopt,
            static_cast<std::int64_t>(entry.length), false};
    }

private:
    [[noreturn]] static void refuse(const char *message) {
        throw std::runtime_error(std::string("reference indexed-storage probe: ") + message);
    }
    static std::int64_t integer(const ny::Value &value) {
        const auto *number = std::get_if<std::int64_t>(&value);
        if (!number) {
            refuse("requires integer operand");
        }
        return *number;
    }
    void charge(std::size_t count) {
        if (count > maximum_array_elements - charged_cells_) {
            refuse("storage cell limit");
        }
        charged_cells_ += count;
    }
    std::size_t reserve(bool array, bool text, std::size_t count, std::size_t stack_size) {
        if (allocations_.size() == maximum_array_literals) {
            refuse("storage descriptor limit");
        }
        charge(count);
        const auto index = allocations_.size();
        allocations_.push_back({array, text, false, count, 0});
        pending_ = observation{observation_kind::allocation, stack_size, index, std::nullopt, false};
        return index;
    }
    allocation &array_entry(const ny::Value &value) {
        const auto *array = std::get_if<ny::ArrayHandle>(&value);
        if (!array) {
            refuse("requires array handle");
        }
        check_array(value);
        return allocations_[identities_.at(array->address)];
    }
    const allocation &receiver(const ny::Value &value, std::optional<std::size_t> buffer) {
        if (buffer) {
            require_buffer(value, *buffer);
            return allocations_[*buffer];
        }
        if (!std::holds_alternative<ny::ArrayHandle>(value)) {
            refuse("indexed receiver lacks array/buffer origin");
        }
        return array_entry(value);
    }
    void mutation(std::size_t stack_size) {
        pending_ = observation{observation_kind::mutation, stack_size, std::nullopt, std::nullopt, false};
    }
    std::vector<allocation> allocations_;
    std::unordered_map<std::int64_t, std::size_t> identities_;
    std::optional<observation> pending_;
    std::size_t charged_cells_ = 0;
    std::uint64_t next_minimum_address_ = 0;
};
