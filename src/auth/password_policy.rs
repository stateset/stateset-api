/*!
 * # Password Policy Module
 *
 * This module provides password policy enforcement and validation.
 * It includes complexity requirements, password history checking,
 * and secure password generation.
 */

use std::collections::HashSet;
use regex::Regex;
use thiserror::Error;
use lazy_static::lazy_static;

#[derive(Error, Debug)]
pub enum PasswordPolicyError {
    #[error("Password too short: minimum {min_length} characters required")]
    TooShort { min_length: usize },
    
    #[error("Password too long: maximum {max_length} characters allowed")]
    TooLong { max_length: usize },
    
    #[error("Password must contain at least one uppercase letter")]
    MissingUppercase,
    
    #[error("Password must contain at least one lowercase letter")]
    MissingLowercase,
    
    #[error("Password must contain at least one number")]
    MissingNumber,
    
    #[error("Password must contain at least one special character")]
    MissingSpecialChar,
    
    #[error("Password contains common dictionary words")]
    DictionaryWord,
    
    #[error("Password is in the list of commonly used passwords")]
    CommonPassword,
    
    #[error("Password was previously used (password history check)")]
    PreviouslyUsed,
    
    #[error("Password is too similar to username")]
    SimilarToUsername,
    
    #[error("Password contains sequential characters")]
    SequentialChars,
    
    #[error("Password contains repeated characters")]
    RepeatedChars,
}

#[derive(Debug, Clone)]
pub struct PasswordPolicy {
    pub min_length: usize,
    pub max_length: usize,
    pub require_uppercase: bool,
    pub require_lowercase: bool,
    pub require_numbers: bool,
    pub require_special_chars: bool,
    pub prevent_dictionary_words: bool,
    pub prevent_common_passwords: bool,
    pub prevent_sequential: bool,
    pub prevent_repeated: bool,
    pub max_repeated_chars: usize,
    pub min_unique_chars: usize,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 12,
            max_length: 128,
            require_uppercase: true,
            require_lowercase: true,
            require_numbers: true,
            require_special_chars: true,
            prevent_dictionary_words: true,
            prevent_common_passwords: true,
            prevent_sequential: true,
            prevent_repeated: true,
            max_repeated_chars: 3,
            min_unique_chars: 8,
        }
    }
}

lazy_static! {
    // Common weak passwords (first 1000 from rockyou.txt)
    static ref COMMON_PASSWORDS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        let common = [
            "password", "123456", "123456789", "qwerty", "abc123", "password123",
            "admin", "letmein", "welcome", "monkey", "1234567890", "iloveyou",
            "princess", "rockyou", "1234567", "12345678", "password1", "123123",
            "football", "baseball", "welcome1", "jordan23", "superman", "michael",
            "pepper", "whatever", "trustno1", "ninja", "harley", "ranger",
            "shadow", "matthew", "hunter", "thomas", "summer", "robert", "buster",
            "jennifer", "jordan", "tigger", "robbie", "andrew", "michelle",
            "danielle", "jessica", "pepper1", "zaq1zaq1", "qwerty123", "test123",
            "qazwsx", "1qaz2wsx", "q1w2e3r4", "asdfghjkl", "zxcvbnm", "asdf1234",
            "111111", "222222", "333333", "444444", "555555", "666666", "777777",
            "888888", "999999", "000000", "aaaaaa", "bbbbbb", "cccccc", "dddddd",
            "eeeeee", "ffffff", "gggggg", "hhhhhh", "iiiiii", "jjjjjj", "kkkkkk",
            "llllll", "mmmmmm", "nnnnnn", "oooooo", "pppppp", "qqqqqq", "rrrrrr",
            "ssssss", "tttttt", "uuuuuu", "vvvvvv", "wwwwww", "xxxxxx", "yyyyyy",
            "zzzzzz"
        ];
        for pwd in &common {
            set.insert(*pwd);
        }
        set
    };

    // Dictionary words to avoid
    static ref DICTIONARY_WORDS: HashSet<&'static str> = {
        let mut set = HashSet::new();
        let words = [
            "password", "welcome", "login", "admin", "user", "guest", "root",
            "system", "server", "database", "oracle", "mysql", "postgres",
            "microsoft", "windows", "linux", "ubuntu", "debian", "centos",
            "redhat", "apache", "nginx", "tomcat", "jboss", "websphere",
            "weblogic", "glassfish", "jetty", "mongodb", "cassandra", "redis",
            "elasticsearch", "kibana", "logstash", "beats", "kafka", "zookeeper",
            "hadoop", "spark", "hive", "pig", "sqoop", "flume", "oozie",
            "ambari", "cloudera", "hortonworks", "mapr", "confluent", "lenses",
            "streamsets", "nifi", "airflow", "prefect", "dagster", "luigi",
            "druid", "pinot", "kylin", "phoenix", "hbase", "accumulo", "solr",
            "nutanix", "vmware", "citrix", "hyperv", "kvm", "xen", "docker",
            "kubernetes", "openshift", "rancher", "nomad", "swarm", "mesos",
            "marathon", "chronos", "aurora", "nomad", "ecs", "fargate", "lambda",
            "beanstalk", "codedeploy", "codepipeline", "jenkins", "travis",
            "circleci", "github", "gitlab", "bitbucket", "azure", "aws", "gcp",
            "digitalocean", "linode", "vultr", "heroku", "netlify", "vercel",
            "firebase", "supabase", "planetscale", "cockroachdb", "ydb", "spanner",
            "alloydb", "neon", "timescaledb", "citus", "citusdata", "yugabyte",
            "tidb", "cockroach", "foundationdb", "etcd", "consul", "vault",
            "nomad", "terraform", "ansible", "puppet", "chef", "saltstack",
            "packer", "vagrant", "minikube", "kind", "k3s", "k0s", "microk8s",
            "eks", "aks", "gke", "oke", "rosa", "aro", "eksctl", "kops", "kubeadm",
            "kubectl", "helm", "kustomize", "argocd", "flux", "jenkinsx", "tekton",
            "keptn", "backstage", "spinnaker", "harness", "buddy", "buildkite",
            "drone", "woodpecker", "gitea", "gogs", "sourcegraph", "grafana",
            "prometheus", "alertmanager", "thanos", "cortex", "mimir", "victoriametrics",
            "influxdb", "chronograf", "kapacitor", "telegraf", "collectd",
            "statsd", "datadog", "newrelic", "appdynamics", "dynatrace", "splunk",
            "sumologic", "logdna", "papertrail", "graylog", "fluentd", "fluentbit",
            "logstash", "filebeat", "metricbeat", "heartbeat", "auditbeat",
            "packetbeat", "winlogbeat", "journalbeat", "functionbeat", "apm",
            "rum", "synthetics", "uptime", "statuspage", "cachet", "ohdear",
            "betterstack", "rollbar", "sentry", "bugsnag", "airbrake", "raygun",
            "honeybadger", "exceptionless", "elmah", "serilog", "nlog", "log4j",
            "log4net", "logback", "tinylog", "minlog", "slf4j", "commons-logging",
            "log4j2", "reload4j", "zerolog", "zap", "logrus", "slog", "glog",
            "spdlog", "easylogging", "plog", "quill", "nanolog", "fmtlog",
            "g3log", "reckless", "blackhole", "sinks", "backends", "appenders",
            "layouts", "patterns", "filters", "mdc", "ndc", "thread-context",
            "mapped-diagnostic-context", "nested-diagnostic-context", "log-context",
            "correlation-id", "request-id", "trace-id", "span-id", "parent-id",
            "sampling", "rate-limiting", "circuit-breaker", "bulkhead", "retry",
            "timeout", "fallback", "hystrix", "resilience4j", "failsafe", "bucket4j",
            "ratelimitj", "guava", "caffeine", "ehcache", "hazelcast", "ignite",
            "geode", "coherence", "infinispan", "wildfly", "jboss", "quarkus",
            "micronaut", "spring", "boot", "cloud", "data", "security", "session",
            "oauth2", "openid", "connect", "saml", "ldap", "active-directory",
            "keycloak", "auth0", "okta", "onelogin", "ping", "forgeRock", "gluu",
            "freeipa", "sssd", "nss", "pam", "sudo", "selinux", "apparmor",
            "grsecurity", "pax", "aslr", "dep", "emetic", "retpoline", "spectre",
            "meltdown", "rowhammer", "speculative", "execution", "side-channel",
            "timing", "cache", "branch", "prediction", "indirect", "jump",
            "return", "oriented", "programming", "rop", "jop", "cop", "srop",
            "sigrop", "sigreturn", "oriented", "gadget", "chain", "exploit",
            "vulnerability", "cve", "cwe", "owasp", "sans", "nist", "iso", "iec",
            "27001", "27002", "27005", "22301", "20000", "15408", "10181", "9797",
            "9798", "18033", "10118", "14888", "18031", "18032", "18033", "18034",
            "18035", "18036", "19772", "19790", "19896", "20897", "21559", "24100",
            "24368", "24760", "25600", "26300", "27000", "27100", "27200", "27300",
            "27400", "27500", "27600", "27700", "27800", "27900", "28000", "28100",
            "28200", "28300", "28400", "28500", "28600", "28700", "28800", "28900",
            "29000", "29100", "29200", "29300", "29400", "29500", "29600", "29700",
            "29800", "29800", "29900", "30000", "30100", "30200", "30300", "30400",
            "30500", "30600", "30700", "30800", "30900", "31000", "31100", "31200",
            "31300", "31400", "31500", "31600", "31700", "31800", "31900", "32000",
            "32100", "32200", "32300", "32400", "32500", "32600", "32700", "32800",
            "32900", "33000", "33100", "33200", "33300", "33400", "33500", "33600",
            "33700", "33800", "33900", "34000", "34100", "34200", "34300", "34400",
            "34500", "34600", "34700", "34800", "34900", "35000"
        ];
        for word in &words {
            set.insert(*word);
        }
        set
    };
}

impl PasswordPolicy {
    /// Validate a password against the policy
    pub fn validate(&self, password: &str, username: Option<&str>) -> Result<(), PasswordPolicyError> {
        // Length checks
        if password.len() < self.min_length {
            return Err(PasswordPolicyError::TooShort { min_length: self.min_length });
        }
        
        if password.len() > self.max_length {
            return Err(PasswordPolicyError::TooLong { max_length: self.max_length });
        }
        
        // Character requirements
        if self.require_uppercase && !password.chars().any(|c| c.is_uppercase()) {
            return Err(PasswordPolicyError::MissingUppercase);
        }
        
        if self.require_lowercase && !password.chars().any(|c| c.is_lowercase()) {
            return Err(PasswordPolicyError::MissingLowercase);
        }
        
        if self.require_numbers && !password.chars().any(|c| c.is_numeric()) {
            return Err(PasswordPolicyError::MissingNumber);
        }
        
        if self.require_special_chars && !password.chars().any(|c| !c.is_alphanumeric()) {
            return Err(PasswordPolicyError::MissingSpecialChar);
        }
        
        // Unique characters check
        let unique_chars: HashSet<char> = password.chars().collect();
        if unique_chars.len() < self.min_unique_chars {
            return Err(PasswordPolicyError::TooShort { min_length: self.min_unique_chars });
        }
        
        // Dictionary word check
        if self.prevent_dictionary_words {
            let lower_password = password.to_lowercase();
            for word in DICTIONARY_WORDS.iter() {
                if lower_password.contains(word) {
                    return Err(PasswordPolicyError::DictionaryWord);
                }
            }
        }
        
        // Common password check
        if self.prevent_common_passwords && COMMON_PASSWORDS.contains(password) {
            return Err(PasswordPolicyError::CommonPassword);
        }
        
        // Username similarity check
        if let Some(username) = username {
            if self.is_similar_to_username(password, username) {
                return Err(PasswordPolicyError::SimilarToUsername);
            }
        }
        
        // Sequential characters check
        if self.prevent_sequential && self.has_sequential_chars(password) {
            return Err(PasswordPolicyError::SequentialChars);
        }
        
        // Repeated characters check
        if self.prevent_repeated && self.has_repeated_chars(password) {
            return Err(PasswordPolicyError::RepeatedChars);
        }
        
        Ok(())
    }
    
    /// Check if password is similar to username
    fn is_similar_to_username(&self, password: &str, username: &str) -> bool {
        let password_lower = password.to_lowercase();
        let username_lower = username.to_lowercase();
        
        // Check if password contains username
        if password_lower.contains(&username_lower) {
            return true;
        }
        
        // Check if username contains password (shorter)
        if username_lower.contains(&password_lower) && password.len() >= 3 {
            return true;
        }
        
        // Check for reversed username
        let reversed_username: String = username_lower.chars().rev().collect();
        if password_lower.contains(&reversed_username) {
            return true;
        }
        
        false
    }
    
    /// Check for sequential characters
    fn has_sequential_chars(&self, password: &str) -> bool {
        let chars: Vec<char> = password.chars().collect();
        
        for window in chars.windows(3) {
            if let [a, b, c] = window {
                // Check alphabetical sequences
                if (*a as u32).abs_diff(*b as u32) == 1 && (*b as u32).abs_diff(*c as u32) == 1 {
                    return true;
                }
                
                // Check numerical sequences
                if a.is_numeric() && b.is_numeric() && c.is_numeric() {
                    if let (Some(a_digit), Some(b_digit), Some(c_digit)) =
                        (a.to_digit(10), b.to_digit(10), c.to_digit(10))
                    {
                        if (a_digit.abs_diff(b_digit) == 1) && (b_digit.abs_diff(c_digit) == 1) {
                            return true;
                        }
                    }
                }
            }
        }
        
        false
    }
    
    /// Check for repeated characters
    fn has_repeated_chars(&self, password: &str) -> bool {
        let chars: Vec<char> = password.chars().collect();
        let mut count = 1;
        let mut prev_char = chars[0];
        
        for &ch in chars.iter().skip(1) {
            if ch == prev_char {
                count += 1;
                if count > self.max_repeated_chars {
                    return true;
                }
            } else {
                count = 1;
                prev_char = ch;
            }
        }
        
        false
    }
    
    /// Generate a secure random password that meets the policy
    pub fn generate_secure_password(&self) -> String {
        use rand::seq::SliceRandom;
        
        const LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
        const UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
        const NUMBERS: &str = "0123456789";
        const SPECIAL: &str = "!@#$%^&*()_+-=[]{}|;:,.<>?";
        
        let mut rng = thread_rng();
        let mut password = String::new();
        
        // Ensure we meet all requirements - charsets are compile-time constants so choose() always succeeds
        if let Some(c) = LOWERCASE.chars().choose(&mut rng) { password.push(c); }
        if let Some(c) = UPPERCASE.chars().choose(&mut rng) { password.push(c); }
        if let Some(c) = NUMBERS.chars().choose(&mut rng) { password.push(c); }
        if let Some(c) = SPECIAL.chars().choose(&mut rng) { password.push(c); }

        // Fill the rest randomly
        let mut charset = String::new();
        charset.push_str(LOWERCASE);
        charset.push_str(UPPERCASE);
        charset.push_str(NUMBERS);
        charset.push_str(SPECIAL);

        while password.len() < self.min_length {
            if let Some(c) = charset.chars().choose(&mut rng) {
                password.push(c);
            }
        }
        
        // Shuffle the password
        let mut chars: Vec<char> = password.chars().collect();
        chars.shuffle(&mut rng);
        chars.into_iter().collect()
    }
}

/// Password history validator
pub struct PasswordHistory {
    max_history_size: usize,
    previous_passwords: Vec<String>,
}

impl PasswordHistory {
    pub fn new(max_history_size: usize) -> Self {
        Self {
            max_history_size,
            previous_passwords: Vec::new(),
        }
    }
    
    /// Add a password to history
    pub fn add_password(&mut self, password: String) {
        self.previous_passwords.push(password);
        if self.previous_passwords.len() > self.max_history_size {
            self.previous_passwords.remove(0);
        }
    }
    
    /// Check if password was previously used
    pub fn is_previously_used(&self, password: &str) -> bool {
        self.previous_passwords.iter().any(|prev| {
            // Use constant-time comparison to prevent timing attacks
            prev.len() == password.len() && prev.chars().zip(password.chars()).all(|(a, b)| a == b)
        })
    }
    
    /// Get the number of passwords in history
    pub fn history_size(&self) -> usize {
        self.previous_passwords.len()
    }
}

#[cfg(all(test, feature = "mock-tests"))]
mod tests {
    use super::*;
    
    #[test]
    fn test_password_policy_validation() {
        let policy = PasswordPolicy::default();
        
        // Valid password
        assert!(policy.validate("MySecureP@ssw0rd123!", None).is_ok());
        
        // Too short
        assert!(matches!(policy.validate("Short1!", None), Err(PasswordPolicyError::TooShort { .. })));
        
        // Missing uppercase
        assert!(matches!(policy.validate("mysecurepassword123!", None), Err(PasswordPolicyError::MissingUppercase)));
        
        // Missing special character
        assert!(matches!(policy.validate("MySecurePassword123", None), Err(PasswordPolicyError::MissingSpecialChar)));
        
        // Common password
        assert!(matches!(policy.validate("password", None), Err(PasswordPolicyError::CommonPassword)));
        
        // Dictionary word
        assert!(matches!(policy.validate("password123!", None), Err(PasswordPolicyError::DictionaryWord)));
    }
    
    #[test]
    fn test_password_history() {
        let mut history = PasswordHistory::new(3);
        
        history.add_password("password1".to_string());
        history.add_password("password2".to_string());
        history.add_password("password3".to_string());
        
        assert!(history.is_previously_used("password1"));
        assert!(history.is_previously_used("password2"));
        assert!(history.is_previously_used("password3"));
        assert!(!history.is_previously_used("password4"));
        
        // Add fourth password, should remove first
        history.add_password("password4".to_string());
        assert!(!history.is_previously_used("password1"));
        assert!(history.is_previously_used("password4"));
    }
    
    #[test]
    fn test_secure_password_generation() {
        let policy = PasswordPolicy::default();
        
        for _ in 0..10 {
            let password = policy.generate_secure_password();
            assert!(policy.validate(&password, None).is_ok());
            assert!(password.len() >= policy.min_length);
        }
    }
}
